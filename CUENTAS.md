# Cuentas sin verificar

**POSPUESTO (24 nov 2026).** Diseño acordado, sin código ni cambios de BD. La app se lanzó esa semana
con un usuario; primero se mide cuánta gente llega desde las IA (ChatGPT ya nos manda clientes).

**Retomarlo cuando** las visitas desde IA (Umami en `analytics.xplaya.com`: fuentes y `utm_campaign=llms-txt`) lleguen a un número que valga la
pena, o cuando alguien sin cuenta pida imprimir o escribirnos a distancia.

## Idea

Hoy la app exige un cliente con teléfono comprobado (kiosko o enlace del recibo,
`APP_MONEDERO.md`). Con el chat propio, el teléfono ya no hace falta para entrar: pasa a ser **lo que
se da a cambio del monedero**.

- Cualquiera instala la app, crea su cuenta (passkey o Google) y puede escribirnos e imprimir. La
  impresión ya la autoriza la caja y se paga al recoger, así que no arriesga nada.
- La IA recomienda un enlace (`llms.txt`); si alguien lo reenvía, el otro celular solo crea su propia
  cuenta sin verificar.
- La caja ve qué tan confiable es cada cliente (`v_confianza_clientes`, hecha el 24 nov 2026 para la
  venta del POS).

## Sin verificar = cliente sin teléfono

- Un renglón en `Clientes` con `Telefono` NULL (el `UNIQUE` admite varios NULL). Chat, impresión,
  fotos y anuncio siguen atados a `ClienteId` sin cambios.
- La búsqueda por teléfono de la caja no los encuentra. El QR de su tarjeta lleva el **Id** en vez del
  teléfono (`src/routes/monedero.rs`, `qr_caja`); el buscador del POS ya acepta un GUID
  (`ClientesRepo.Search`).
- El NULL se resuelve en SQL; el C# del POS no cambia:
  - Al leer, `COALESCE(Telefono,'')` en `ClientesRepo` (`Cols`; hoy `dr.GetString` revienta con NULL) y
    en los JOIN de `TrabajosImpresionRepo` y `AnunciosRepo`. Las vistas ya ocultan WhatsApp si viene vacío.
  - Al guardar, `NULLIF(@Telefono,'')`: dos `''` chocarían en el `UNIQUE`.
  - Las tarjetas impresas excluyen a quien no tiene teléfono.
- En xplaya, `telefono` pasa a `Option<String>` donde se lee (QR, permiso de fotos, avisos, saldo).

## Crear cuenta

- Botón **Crear cuenta** en la portada de la app: nombre o alias, términos, passkey o Google.
- Reusa `AltaPendiente` y `completar_alta` (`src/routes/alta.rs`) con un `Quien::SinVerificar` y un
  `Origen::App`; `ClienteCelulares.Origen` admite `'app'`.

## Cashback guardado

- La venta a una cuenta sin verificar genera monedero como cualquier otra y **vence desde la compra**
  (no se recalcula nada).
- El POS lo muestra como "guardado, se usa al verificarse" y no deja canjearlo sin teléfono.
- La app lo anuncia: "Tienes $X guardados. Verifica tu cuenta para usarlos."

## Verificar

**Desde la app:**

1. Escribe su teléfono.
2. Se crea una `KioskoSolicitudes` con columnas nuevas `Origen ('kiosko','app')` y `CuentaId` (la
   cuenta que la pidió). El código sale como el del kiosko: en el aviso flotante del POS (Copiar
   código) y por ntfy (`avisos.alta_kiosko`). La caja lo manda a mano por WhatsApp.
3. Lo escribe en la **misma sesión**; se exige `CuentaId` = cliente de la sesión. El código prueba el
   número; la sesión prueba el aparato.
4. Si el teléfono es nuevo: `UPDATE clientes SET telefono`. Si ya es de otro cliente:
   `fn_fusionar_clientes(temporal, definitivo)`.

**En la caja**, con el cliente en persona: "Verificar: capturar teléfono" en Editar cliente, sin
código. Si el número ya existe, la misma fusión.

**`fn_fusionar_clientes`** (en `reportes.sql` del POS): mueve `Ajustes` (el monedero va con ellas),
`Pedidos`, `Mensajes`, `TrabajosImpresion`, `PedidosFotos`, `Notificaciones` y `ClienteCelulares`
(el celular activo del definitivo queda `'reemplazado'`, como en `ligar`). `Anuncios` es único por
cliente: se queda el del definitivo. Copia `NombreChat` y `FotoPerfil` si faltan y borra el
temporal. Al escribirla, recorrer **todos** los FK a `Clientes` del esquema.

## Por decidir al retomar

Pueden ser complejidad que no hace falta; decidir con datos:

- Límites de cuentas nuevas por IP (en memoria).
- Vigencia del código desde la app (¿24 h en vez de 15 min, por si lo piden de noche?).
- Si el chat (abierto a todos los clientes desde el 23 nov 2026) se abre también a las cuentas sin verificar.
- Un nivel de "crédito" en la confianza.
