# Plan de categorías — de caos a navegación útil

Afecta a tres proyectos: `inventario_papeleria` (esquema de BD y POS), `xplaya` (catálogo web) y el kiosko (`PLAN_KIOSKO.md`).

---

## Situación actual

**273 "categorías"** entradas manualmente en el POS, sin estándar. No son categorías de navegación — son etiquetas de tipo de producto que alguien fue escribiendo al dar de alta cada artículo. Problemas concretos:

- Duplicados con typos: MONOGRAFÍA / MONOGRAFIA / MONOGRAF`?`A (error de encoding)
- Duplicados con variantes: PLUMA / PLUMON / PLUMONES / PLUMÓN / PLUMIN (5 entradas para lo mismo)
- Granularidad de producto, no de cliente: CUADERNO COSIDO PROFESIONAL es un SKU, no una categoría
- Servicios mezclados: REMISION, CONTRATO, PAGARE, INTERNET no son productos navegables
- Espacios en blanco sobrantes: "GOMA " y "GOMA" son distintas en la BD

**Consecuencia:** inutilizables como tiles de navegación en el kiosko y en xplaya. Un cliente no puede elegir entre 273 opciones. Un desarrollador tampoco puede mapearlas a íconos sin antes limpiarlas.

---

## La solución: categorías derivadas de datos, no inventadas

El **EMBEDDINGS_PLAN.md** (`inventario_papeleria/EMBEDDINGS_PLAN.md`) genera dos cosas relevantes:

1. **FamiliasSemanticas** (~40 clusters por k-means) — agrupaciones coherentes calculadas por similitud semántica del nombre del producto. Sin ambigüedad: "pluma", "bolígrafo" y "lapicero" quedan juntos porque los vectores son similares.

2. **Búsqueda semántica como fallback** — cuando tsvector devuelve 0 resultados, se hace búsqueda vectorial. El cliente del kiosko escribe "plumón rojo" y encuentra "MARCADOR PUNTA FINA ROJO 12PZ" aunque los tokens no coincidan.

Este plan toma las ~40 `FamiliasSemanticas` y las colapsa en **6-8 `MacroCategorias`** navegables para el cliente. El trabajo manual se reduce a revisar y nombrar — los clusters hacen la agrupación pesada.

---

## Relación con los otros planes

```
EMBEDDINGS_PLAN Fase 0+1   →  embeddings generados (prerequisito de todo)
EMBEDDINGS_PLAN Fase 2     →  FamiliasSemanticas (~40 clusters)
                                        ↓
                           CATEGORIAS_PLAN Fase 1   →  revisión humana
                           CATEGORIAS_PLAN Fase 2   →  MacroCategorias en BD
                           CATEGORIAS_PLAN Fase 3   →  tiles en kiosko
                           CATEGORIAS_PLAN Fase 4   →  filtros en xplaya.com
                                        
EMBEDDINGS_PLAN Fase 4     →  búsqueda semántica en xplaya + kiosko
                               (independiente de las categorías — mejora el buscador,
                                no los tiles de navegación)
```

**Lo que no bloquea el kiosko:** PLAN_KIOSKO Fases 1-3 (búsqueda + low-sellers + pedido) pueden lanzarse sin categorías. Los tiles de navegación se agregan después, una vez que las `MacroCategorias` estén validadas.

---

## Resumen de fases

| # | Fase | Prerequisito | Estado |
|---|------|-------------|--------|
| 0 | Prerequisito: embeddings generados | EMBEDDINGS_PLAN Fase 0+1 completas | ⬜ |
| 1 | Revisión del clustering y diseño de macro-categorías | Fase 0 | ⬜ |
| 2 | Esquema en BD + mapeo | Fase 1 | ⬜ |
| 3 | Tiles de navegación en kiosko | PLAN_KIOSKO Fase 1 + Fase 2 aquí | ⬜ |
| 4 | Filtros por categoría en xplaya.com | Fase 2 | ⬜ |

---

## Fase 0 — Prerequisito: embeddings generados

Completar **EMBEDDINGS_PLAN Fases 0 y 1**:
- `CREATE EXTENSION vector` + columnas en `Productos` (`embedding`, `EmbeddingGeneratedAt`)
- Job de ingest corriendo: todos los productos tienen embedding

Esto no es trabajo de este plan — está detallado en `EMBEDDINGS_PLAN.md`. Se lista aquí solo para dejar claro el orden.

**Verificación:**
- [ ] `SELECT COUNT(*) FILTER (WHERE embedding IS NOT NULL) FROM Productos` = 100%

---

## Fase 1 — Revisión del clustering y diseño de macro-categorías

### Paso 1: correr el clustering

Ejecutar **EMBEDDINGS_PLAN Fase 2** con `k=40`. El script `cluster.py` genera `FamiliasSemanticas` con los ~40 grupos y un reporte de los 10 productos más representativos de cada uno.

### Paso 2: revisar el reporte con ojo de dueño

El reporte tiene esta forma (ejemplo):
```
Familia 1 (87 productos): CUADERNO PROFESIONAL 100H | LIBRETA PASTA DURA | CUADERNO COSIDO | BLOCK PROFESIONAL ...
Familia 2 (64 productos): PLUMA PUNTO FINO AZUL | BOLIGRAFO BIC CRISTAL | LAPICERO GEL | PLUMA BORRABLE ...
Familia 3 (51 productos): MARCADOR PERMANENTE | PLUMON PUNTA FINA | MARCATEXTO AMARILLO | MARCADOR PIZARRON ...
...
```

La pregunta para cada familia: **¿un cliente del kiosko tocaría este tile buscando estos productos?**

### Paso 3: definir las 6-8 macro-categorías

Las familias semánticas tienden a ser demasiado granulares (~40) para ser tiles táctiles. Se colapsan en macro-categorías de cliente. Propuesta inicial basada en el catálogo observado:

| Macro-categoría | Familias semánticas esperadas |
|---|---|
| ✏️ Escritura | plumas, lápices, marcadores, plumones, colores, gomas, sacapuntas |
| 📄 Papel | cuadernos, libretas, hojas, cartulinas, blocks, monografías |
| 📐 Escolar | juegos geométricos, mapas, calculadoras, reglas, compases |
| 📁 Oficina | folders, carpetas, clips, engrapadoras, perforadoras, cinta, pegamento, tijeras |
| 🎨 Arte | foamy, pintura, pinceles, plastilina, diamantina, washi tape, acuarela |
| 🧵 Mercería | hilos, cierres, listones, broches, agujas, aretes, bisutería |
| 🎈 Fiestas | globos, moños, bolsas de regalo, confetti, cascabeles |
| 🖨️ Servicios | copias, impresiones, trámites, engargolado, enmicado |

Estas son una hipótesis — los nombres y agrupaciones finales los define la revisión del clustering real.

**Verificación:**
- [ ] Reporte de clustering generado y revisado
- [ ] 6-8 macro-categorías definidas con nombre e ícono (Font Awesome)
- [ ] Cada `FamiliaSemantica` asignada a una `MacroCategoria`

---

## Fase 2 — Esquema en BD + mapeo

### Tabla nueva: `MacroCategorias`

```sql
CREATE TABLE IF NOT EXISTS MacroCategorias (
    Id      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    Nombre  VARCHAR(100) NOT NULL,   -- "Escritura", "Papel", etc.
    Icono   VARCHAR(50)  NOT NULL,   -- clase Font Awesome: "fa-pen", "fa-file", etc.
    Orden   SMALLINT    NOT NULL DEFAULT 0  -- orden de aparición en el kiosko
);
```

### Columna nueva en `FamiliasSemanticas`

```sql
ALTER TABLE FamiliasSemanticas
    ADD COLUMN IF NOT EXISTS MacroCategoriaId UUID
    REFERENCES MacroCategorias(Id) ON DELETE SET NULL;
```

Esto une la cadena: `Productos.FamiliaSemanticaId → FamiliasSemanticas.MacroCategoriaId → MacroCategorias`.

No se toca `Categorias` ni `CategoriasProductos` — esas tablas siguen existiendo para el POS interno; este esquema es paralelo y solo para navegación pública.

### Inserts de datos

Una vez definidas las macro-categorías en Fase 1, insertar con UUIDs fijos (para que sean reproducibles en migraciones):

```sql
INSERT INTO MacroCategorias (Id, Nombre, Icono, Orden) VALUES
    (gen_random_uuid(), 'Escritura', 'fa-pen',        1),
    (gen_random_uuid(), 'Papel',     'fa-file',       2),
    (gen_random_uuid(), 'Escolar',   'fa-ruler',      3),
    (gen_random_uuid(), 'Oficina',   'fa-folder',     4),
    (gen_random_uuid(), 'Arte',      'fa-palette',    5),
    (gen_random_uuid(), 'Mercería',  'fa-scissors',   6),
    (gen_random_uuid(), 'Fiestas',   'fa-gift',       7),
    (gen_random_uuid(), 'Servicios', 'fa-print',      8);
-- (ajustar tras Fase 1)
```

### Mapeo `FamiliasSemanticas → MacroCategorias`

```sql
UPDATE FamiliasSemanticas
SET MacroCategoriaId = '<uuid-de-escritura>'
WHERE Id IN ('<familia-plumas>', '<familia-marcadores>', '<familia-lapices>', ...);
-- repetir para cada macro-categoría
```

Este paso se hace tras la revisión humana del clustering.

### Reflejo en el script de inicialización

Agregar todo lo anterior a `inventario_papeleria/Ro.Inventario.Core/dbscripts/postgresql_inventario.sql` para que el script de init quede en sincronía.

**Verificación:**
- [ ] `MacroCategorias` creada con 6-8 filas
- [ ] Columna `MacroCategoriaId` en `FamiliasSemanticas`
- [ ] Todas las familias mapean a una macro-categoría (ningún NULL)
- [ ] Script de init actualizado

---

## Fase 3 — Tiles de navegación en kiosko

### Query para el landing del kiosko

```sql
SELECT mc.id, mc.nombre, mc.icono, mc.orden,
       COUNT(DISTINCT p.nid) AS total_productos
FROM macrocategorias mc
JOIN familiassemanticas fs ON fs.macrocategoriaid = mc.id
JOIN productos p ON p.familiaSemanticaid = fs.id
WHERE p.activo = true
GROUP BY mc.id, mc.nombre, mc.icono, mc.orden
ORDER BY mc.orden;
```

### Archivos nuevos en xplaya

- `src/db/kiosko.rs` — agregar `macro_categorias(pool)` con la query anterior
- `src/db/kiosko.rs` — agregar `kiosko_lista_por_categoria(pool, macro_categoria_id, busqueda, pagina, content_base_url)`
- `src/routes/kiosko.rs` — handler `GET /kiosko/categoria/{id}` → grid filtrado por macro-categoría
- `templates/kiosko/lista.html` — agregar franja de tiles táctiles entre el buscador y los low-sellers
- `templates/kiosko/partials/tiles_categorias.html` — fragmento: grilla de tiles (icono grande + nombre)

### UX del tile

```
┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐
│   ✏️    │  │   📄    │  │   📐    │  │   📁    │
│Escritura│  │  Papel  │  │ Escolar │  │ Oficina │
│  (234)  │  │  (189)  │  │  (97)   │  │  (143)  │
└─────────┘  └─────────┘  └─────────┘  └─────────┘
```

- Tap en tile → navega a `/kiosko/categoria/{id}` → grid de productos de esa categoría
- Buscador en la parte superior del kiosko permanece visible en todo momento
- Botón "← Todas" para volver al landing

**Verificación:**
- [ ] Los tiles aparecen en el landing del kiosko
- [ ] Tap en tile filtra correctamente los productos
- [ ] El número de productos por categoría es coherente
- [ ] El buscador sigue funcionando dentro de una categoría
- [ ] `clippy` sin warnings

---

## Fase 4 — Filtros por categoría en xplaya.com

Una vez que las `MacroCategorias` están validadas en el kiosko (datos reales de uso), se agregan como filtros en `/` de xplaya:

- Barra horizontal de chips/tags sobre el grid: "Todos | Escritura | Papel | Escolar | ..."
- Tap/click en chip → HTMX recarga `#catalogo` con `?categoria={id}`
- `src/db/productos.rs` — extender `busqueda()` para aceptar `categoria_id: Option<Uuid>`
- `src/routes/productos.rs` — leer `?categoria=` del query string

**Nota:** esta fase se hace *después* de observar el uso real de las categorías en el kiosko. Si resulta que los clientes no usan los tiles y solo buscan texto, se reconsidira si vale la pena en la web. Los datos del kiosko deciden.

**Verificación:**
- [ ] Los chips de categoría aparecen en xplaya.com
- [ ] Filtrar por categoría + búsqueda de texto funciona en combinación
- [ ] `clippy` sin warnings

---

## ¿Qué resuelven los embeddings que las categorías no resuelven?

Son complementarios, no alternativos:

| Problema | Solución |
|---|---|
| Cliente escribe "plumón" y el catálogo dice "MARCADOR PUNTA FINA" | **Búsqueda semántica** (EMBEDDINGS_PLAN Fase 4) |
| Cliente no sabe qué escribir y quiere navegar tocando | **MacroCategorias** (este plan) |
| 273 categorías caóticas en el POS | **FamiliasSemanticas** como nueva capa de orden |
| Análisis de costos/márgenes por tipo de producto | **FamiliasSemanticas** para reportes (EMBEDDINGS_PLAN Caso 1) |

La búsqueda semántica en el kiosko (EMBEDDINGS_PLAN Fase 4, Caso 4) **no requiere categorías** y puede activarse en paralelo o antes. Son dos mejoras independientes que se complementan.

---

## Camino crítico

```
EMBEDDINGS_PLAN Fase 0+1  (ingest de embeddings)
        ↓
EMBEDDINGS_PLAN Fase 2    (clustering → FamiliasSemanticas)
        ↓
CATEGORIAS_PLAN Fase 1    (revisión humana → definir MacroCategorias)
        ↓
CATEGORIAS_PLAN Fase 2    (BD: tabla + mapeo)
        ↓
CATEGORIAS_PLAN Fase 3    (tiles en kiosko)    ←→    CATEGORIAS_PLAN Fase 4 (xplaya, después)
```

**Mientras tanto, en paralelo y sin esperar este plan:**
- PLAN_KIOSKO Fases 1-3 pueden arrancar hoy (búsqueda tsvector + low-sellers + pedido)
- EMBEDDINGS_PLAN Fase 4 puede activarse en xplaya y kiosko sin categorías

---

## Lo que no hace este plan

- **No limpia las 273 categorías del POS.** Las deja como están — sirven para el uso interno del POS. Si en algún momento se decide limpiarlas, el reporte de auditoría del EMBEDDINGS_PLAN (Caso 1b) lo facilita, pero no es prerequisito de nada aquí.
- **No elimina `Categorias` ni `CategoriasProductos`.** El esquema nuevo es paralelo, no reemplaza.
- **No define qué productos van en cada categoría producto por producto.** Eso lo hace el clustering automáticamente vía `FamiliaSemanticaId`.
