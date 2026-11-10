//! Lógica pura del kiosko de la tienda (las rutas están en `routes/kiosko.rs`).
//!
//! El QR de la pantalla cambia cada 30 segundos: lleva un código firmado con el token del
//! kiosko y el número de ventana de tiempo. Escanearlo prueba que la persona está frente a
//! la pantalla. El servidor no guarda nada: recalcula el código y lo compara.

use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

/// Segundos que dura cada QR.
pub const SEGUNDOS_QR: u64 = 30;

/// Ventana de tiempo actual y milisegundos que le faltan para cambiar.
pub fn ventana_actual() -> (u64, u64) {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let ventana_ms = SEGUNDOS_QR * 1000;
    (ms / ventana_ms, ventana_ms - ms % ventana_ms)
}

/// Código del QR para una ventana: 12 caracteres hex del HMAC-SHA256.
pub fn codigo(token: &str, ventana: u64) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("HMAC acepta cualquier llave");
    mac.update(format!("kiosko:{ventana}").as_bytes());
    let bytes = mac.finalize().into_bytes();
    bytes.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// El código vale en su ventana y en la anterior: quien lo escanea al final de los 30 s
/// alcanza a abrirlo.
pub fn codigo_valido(token: &str, recibido: &str, ventana: u64) -> bool {
    if token.is_empty() {
        return false;
    }
    [ventana, ventana.saturating_sub(1)]
        .iter()
        .any(|v| codigo(token, *v) == recibido)
}

/// Texto del QR de WiFi (formato que leen las cámaras de iPhone y Android). `\ ; , : "` se
/// escapan con `\`.
pub fn texto_qr_wifi(ssid: &str, password: &str) -> String {
    let escapar = |s: &str| {
        s.chars()
            .flat_map(|c| match c {
                '\\' | ';' | ',' | ':' | '"' => vec!['\\', c],
                _ => vec![c],
            })
            .collect::<String>()
    };
    if password.is_empty() {
        format!("WIFI:T:nopass;S:{};;", escapar(ssid))
    } else {
        format!("WIFI:T:WPA;S:{};P:{};;", escapar(ssid), escapar(password))
    }
}

/// "****1234": lo que se muestra del teléfono en el celular, mientras espera el código.
pub fn mascara_telefono(telefono: &str) -> String {
    let digitos: Vec<char> = telefono.chars().filter(|c| c.is_ascii_digit()).collect();
    let ultimos: String = digitos[digitos.len().saturating_sub(4)..].iter().collect();
    format!("****{ultimos}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigo_vale_en_su_ventana_y_la_anterior() {
        let c = codigo("secreto", 100);
        assert!(codigo_valido("secreto", &c, 100));
        assert!(codigo_valido("secreto", &c, 101));
        assert!(!codigo_valido("secreto", &c, 102));
        assert!(!codigo_valido("secreto", &c, 99));
    }

    #[test]
    fn otro_token_no_sirve() {
        let c = codigo("secreto", 100);
        assert!(!codigo_valido("otro", &c, 100));
        assert_eq!(c.len(), 12);
    }

    #[test]
    fn sin_token_nada_vale() {
        assert!(!codigo_valido("", &codigo("", 100), 100));
    }

    #[test]
    fn qr_wifi_escapa_lo_especial() {
        assert_eq!(texto_qr_wifi("mi-red.com", "clave1!"), "WIFI:T:WPA;S:mi-red.com;P:clave1!;;");
        assert_eq!(texto_qr_wifi("a;b", r#"c:"d"#), r#"WIFI:T:WPA;S:a\;b;P:c\:\"d;;"#);
        assert_eq!(texto_qr_wifi("abierta", ""), "WIFI:T:nopass;S:abierta;;");
    }

    #[test]
    fn mascara_solo_ultimos_4() {
        assert_eq!(mascara_telefono("9841234567"), "****4567");
        assert_eq!(mascara_telefono("12"), "****12");
    }
}
