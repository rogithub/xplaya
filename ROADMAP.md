# ROADMAP — Kiosko + Categorías + Embeddings

> **Cómo usar este archivo:**
> Al iniciar una sesión nueva, lee este archivo primero. Encuentra el primer ⬜ de la lista, lee el plan detallado que referencia, y arranca. Al terminar un paso, cámbialo a ✅ y anota brevemente qué quedó hecho.

---

## Contexto en una línea

Papelería física en Playa del Carmen. Raspberry Pi + Elo 2270L (22" touch 1080p) llegan en ~2 semanas. El objetivo es tener el kiosko funcionando el día que llegue el hardware. En paralelo se construye la infraestructura de embeddings que mejorará la búsqueda y generará las categorías.

**Proyectos involucrados:**
- `/mnt/storage/data/code/xplaya` — servidor Rust+Axum, el kiosko vive aquí
- `/mnt/storage/data/code/inventario_papeleria` — POS, BD PostgreSQL compartida

**Planes de detalle:**
- `xplaya/PLAN_KIOSKO.md` — diseño completo del kiosko
- `xplaya/CATEGORIAS_PLAN.md` — cómo generar categorías limpias desde embeddings
- `inventario_papeleria/EMBEDDINGS_PLAN.md` — embeddings + clustering + búsqueda semántica

---

## Lista de trabajo ordenada

### Semana 1 — Kiosko funcional + embeddings corriendo

- ✅ Definir plan del kiosko (`PLAN_KIOSKO.md`)
- ✅ Analizar categorías actuales (273 entradas caóticas, inutilizables)
- ✅ Definir plan de categorías (`CATEGORIAS_PLAN.md`)
- ⬜ **EMBEDDINGS Fase 0** — cambios en BD: pgvector + columnas `embedding`, `EmbeddingGeneratedAt`, `FamiliasSemanticas`, trigger de invalidación → ver `EMBEDDINGS_PLAN.md` sección "Cambios en la base de datos"
- ⬜ **EMBEDDINGS Fase 1** — repo `inventario-embeddings-job`, `ingest.py`, CronJob k3s → ver `EMBEDDINGS_PLAN.md` Fase 1. Meta: 100% de productos con embedding.
- ⬜ **KIOSKO Fase 1** — ruta `/kiosko`, layout táctil sin nav/footer, query de baja venta, branding xplaya.com → ver `PLAN_KIOSKO.md` Fase 1
- ⬜ **KIOSKO Fase 2** — detalle de producto táctil, carrito con botones +/− → ver `PLAN_KIOSKO.md` Fase 2
- ⬜ **KIOSKO Fase 3** — `POST /kiosko/pedidos` con token, pedido en BD con Origen=0, pantalla de confirmación → ver `PLAN_KIOSKO.md` Fase 3
- ⬜ **KIOSKO Fase 4** — eventos Umami: búsqueda, producto visto, carrito, pedido → ver `PLAN_KIOSKO.md` Fase 4

### Semana 2 — Categorías listas antes de que llegue el hardware

- ⬜ **EMBEDDINGS Fase 2** — correr clustering k=40, generar reporte de familias → ver `EMBEDDINGS_PLAN.md` Fase 2
- ⬜ **CATEGORIAS Fase 1** — revisar reporte: definir 6-8 MacroCategorias con nombre e ícono → ver `CATEGORIAS_PLAN.md` Fase 1. _Requiere decisión del dueño._
- ⬜ **CATEGORIAS Fase 2** — crear tabla `MacroCategorias` en BD, mapear familias → ver `CATEGORIAS_PLAN.md` Fase 2
- ⬜ **KIOSKO Fase 5** — consulta de monedero desde el kiosko → ver `PLAN_KIOSKO.md` Fase 5

### Cuando llegue el hardware (Elo + Raspberry Pi)

- ⬜ **CATEGORIAS Fase 3** — tiles de categorías en el landing del kiosko → ver `CATEGORIAS_PLAN.md` Fase 3
- ⬜ **KIOSKO Fase 6** — configurar Raspberry Pi: Chromium kiosk mode, autostart, touch calibration → ver `PLAN_KIOSKO.md` Fase 6
- ⬜ **KIOSKO Fase 7** — deploy en k3s, SealedSecret del `KIOSKO_TOKEN`, prueba end-to-end → ver `PLAN_KIOSKO.md` Fase 7

### Después (sin fecha)

- ⬜ **EMBEDDINGS Fase 4** — búsqueda semántica en xplaya + kiosko (fallback tsvector→vector) → ver `EMBEDDINGS_PLAN.md` Fase 4
- ⬜ **CATEGORIAS Fase 4** — filtros por categoría en xplaya.com → ver `CATEGORIAS_PLAN.md` Fase 4. _Hacerlo solo si los datos del kiosko muestran que los tiles se usan._

---

## Notas de sesiones anteriores

_Agregar aquí cualquier decisión tomada, problema encontrado, o contexto que la siguiente sesión necesite saber. Entradas más recientes arriba._

- **2026-06-30** — Planes definidos. Hardware en camino desde EE.UU., llega en ~2 semanas. Arrancar por EMBEDDINGS Fase 0 o KIOSKO Fase 1 (cualquiera de los dos desbloquea trabajo útil).
