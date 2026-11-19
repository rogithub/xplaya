//! Archivos ya subidos a MinIO que esperan su mensaje.
//!
//! Para mandar 60 fotos, el celular sube cada una con `POST /chat/archivo` (3 a la vez) y al
//! final manda `POST /chat/mensaje` con los números que le dieron. Mientras tanto el `Adjunto`
//! vive aquí, en memoria (xplaya corre con una réplica): lo arma el servidor, así el celular no
//! puede inventar el tamaño, las páginas ni las llaves.
//!
//! Al publicar se marcan, no se quitan: si la respuesta no le llegó al celular y lo vuelve a
//! mandar, sigue encontrando sus archivos (NATS no lo guarda dos veces por el `Nats-Msg-Id`).
//! Después de una hora `vencidas` saca todo; las que nunca se publicaron se borran de MinIO.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio::sync::Semaphore;
use uuid::Uuid;

use super::contrato::Adjunto;

/// Lo que espera un archivo a su mensaje antes de que se borre.
pub const VIDA: Duration = Duration::from_secs(60 * 60);

struct Subida {
    cliente: Uuid,
    mensaje: Uuid,
    adjunto: Adjunto,
    creada: Instant,
    publicada: bool,
}

pub struct Subidas {
    mapa: Mutex<HashMap<Uuid, Subida>>,
    /// Miniaturas y conteo de páginas: abren el archivo en memoria, uno a la vez para no pasar
    /// de la memoria del pod. Escribir la subida a disco no espera.
    pub pesado: Semaphore,
    /// Gotenberg convierte un Word a la vez.
    pub gotenberg: Semaphore,
}

impl Default for Subidas {
    fn default() -> Self {
        Self { mapa: Mutex::new(HashMap::new()), pesado: Semaphore::new(1), gotenberg: Semaphore::new(1) }
    }
}

impl Subidas {
    pub fn guardar(&self, archivo: Uuid, cliente: Uuid, mensaje: Uuid, adjunto: Adjunto) {
        self.guardar_en(archivo, cliente, mensaje, adjunto, Instant::now());
    }

    fn guardar_en(&self, archivo: Uuid, cliente: Uuid, mensaje: Uuid, adjunto: Adjunto, creada: Instant) {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        mapa.insert(archivo, Subida { cliente, mensaje, adjunto, creada, publicada: false });
    }

    /// Los adjuntos de esos archivos, en ese orden, sin sacarlos (si publicar falla, el celular
    /// lo vuelve a intentar con los mismos). Todos tienen que ser de ese cliente y ese mensaje.
    pub fn ver(&self, cliente: Uuid, mensaje: Uuid, archivos: &[Uuid]) -> Option<Vec<Adjunto>> {
        let mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        archivos
            .iter()
            .map(|a| mapa.get(a).filter(|s| s.cliente == cliente && s.mensaje == mensaje).map(|s| s.adjunto.clone()))
            .collect()
    }

    /// Ya están en un mensaje publicado: al vencer no se borran de MinIO.
    pub fn publicadas(&self, archivos: &[Uuid]) {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        for a in archivos {
            if let Some(s) = mapa.get_mut(a) {
                s.publicada = true;
            }
        }
    }

    /// Saca las que llevan más de `VIDA`; regresa las que nunca se publicaron, para borrarlas de
    /// MinIO.
    pub fn vencidas(&self) -> Vec<Adjunto> {
        self.vencidas_en(Instant::now())
    }

    fn vencidas_en(&self, ahora: Instant) -> Vec<Adjunto> {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        let viejas: Vec<Uuid> =
            mapa.iter().filter(|(_, s)| ahora.duration_since(s.creada) >= VIDA).map(|(id, _)| *id).collect();
        viejas.iter().filter_map(|id| mapa.remove(id)).filter(|s| !s.publicada).map(|s| s.adjunto).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENTE: Uuid = Uuid::from_u128(1);
    const MENSAJE: Uuid = Uuid::from_u128(2);

    fn adjunto(nombre: &str) -> Adjunto {
        Adjunto {
            nombre: nombre.into(),
            mime: "image/jpeg".into(),
            bytes: 1,
            llave: format!("{CLIENTE}/{MENSAJE}/{nombre}"),
            paginas: None,
            pdf: None,
            miniatura: None,
        }
    }

    #[test]
    fn se_ven_en_el_orden_pedido_aunque_ya_se_publicaron() {
        let s = Subidas::default();
        let (a, b) = (Uuid::from_u128(10), Uuid::from_u128(11));
        s.guardar(a, CLIENTE, MENSAJE, adjunto("a.jpg"));
        s.guardar(b, CLIENTE, MENSAJE, adjunto("b.jpg"));
        let vistos = s.ver(CLIENTE, MENSAJE, &[b, a]).unwrap();
        assert_eq!(vistos.iter().map(|x| x.nombre.as_str()).collect::<Vec<_>>(), ["b.jpg", "a.jpg"]);
        assert!(s.ver(CLIENTE, MENSAJE, &[a]).is_some(), "ver no las saca");
        s.publicadas(&[a, b]);
        assert!(s.ver(CLIENTE, MENSAJE, &[a, b]).is_some(), "un reintento las sigue encontrando");
    }

    #[test]
    fn solo_las_del_cliente_y_el_mensaje() {
        let s = Subidas::default();
        let a = Uuid::from_u128(10);
        s.guardar(a, CLIENTE, MENSAJE, adjunto("a.jpg"));
        assert_eq!(s.ver(Uuid::from_u128(99), MENSAJE, &[a]), None, "otro cliente");
        assert_eq!(s.ver(CLIENTE, Uuid::from_u128(99), &[a]), None, "otro mensaje");
        assert_eq!(s.ver(CLIENTE, MENSAJE, &[a, Uuid::from_u128(12)]), None, "si falta una, ninguna");
    }

    #[test]
    fn las_viejas_salen_para_borrarlas() {
        let s = Subidas::default();
        let ahora = Instant::now();
        s.guardar_en(Uuid::from_u128(10), CLIENTE, MENSAJE, adjunto("vieja.jpg"), ahora);
        s.guardar_en(Uuid::from_u128(11), CLIENTE, MENSAJE, adjunto("nueva.jpg"), ahora + VIDA / 2);
        s.guardar_en(Uuid::from_u128(12), CLIENTE, MENSAJE, adjunto("mandada.jpg"), ahora);
        s.publicadas(&[Uuid::from_u128(12)]);
        let vencidas = s.vencidas_en(ahora + VIDA);
        assert_eq!(vencidas.iter().map(|x| x.nombre.as_str()).collect::<Vec<_>>(), ["vieja.jpg"], "la mandada no se borra");
        assert_eq!(s.ver(CLIENTE, MENSAJE, &[Uuid::from_u128(12)]), None, "pero ya no espera");
        assert!(s.vencidas_en(ahora + VIDA).is_empty(), "ya salió");
        assert!(s.ver(CLIENTE, MENSAJE, &[Uuid::from_u128(11)]).is_some());
    }
}
