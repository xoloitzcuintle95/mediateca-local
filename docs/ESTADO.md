# Estado del proyecto — Mediateca Local

Commit de referencia: **`747f331`** (`Endurecer portadas, raíz y rutas no UTF-8`) más el `fadvise` de lectura de etiquetas, todavía sin commit. Las cifras de la colección son mediciones puntuales y el disco cambió durante la sesión.

Límite de este documento: **200 líneas**.

## 1. Qué es y cómo correrla

App de escritorio para COSMIC en Rust + libcosmic. Cataloga archivos locales de música, video y libros; no modifica la colección.

- Nombre visible: **Mediateca Local**
- Crate/binario: `mediateca-local`
- ID: `com.xoloitzcuintle95.MediatecaLocal`, definido solo en `src/model.rs`
- Datos: `~/.local/share/com.xoloitzcuintle95.MediatecaLocal/catalogo.db`
- Caché: `~/.cache/com.xoloitzcuintle95.MediatecaLocal/covers/`
- Configuración: `~/.config/com.xoloitzcuintle95.MediatecaLocal/` (cosmic-config; `catalog_root` vacío hasta elegir carpeta)

Comandos: `cargo check`, `cargo clippy`, `just build-release`. `just run` lo ejecuta el usuario, no el agente. La GUI no se abrió durante esta revisión.

## 2. Módulos

| Archivo | Líneas | Responsabilidad |
|---|---:|---|
| `src/main.rs` | 34 | Arranque, i18n y `cosmic::app::run`. |
| `src/app.rs` | 1250 | Modelo, mensajes, rutas, selector de carpeta, carga inicial, escaneo, apertura y drawers. |
| `src/config.rs` | 21 | `catalog_root` en cosmic-config, versión 2; default vacío. |
| `src/covers.rs` | 744 | Barras, recorte/encaje, miniaturas JPEG v2, caché y límites de portada. |
| `src/db.rs` | 1451 | SQLite, migraciones, upsert, borrado seguro, alias, enrich de música y EPUB. |
| `src/epub.rs` | 302 | Lectura de título, autores y portada EPUB con límites, solo lectura. |
| `src/i18n.rs` | 101 | Carga Fluent; `fl!` y `get(key)`. |
| `src/library.rs` | 584 | Agrupación musical artista → álbum → canción. |
| `src/media.rs` | 116 | Tabla única de tipos: extensiones, aspecto, jerarquía, apertura. |
| `src/model.rs` | 212 | `Kind`, metadata, `MediaItem`, resumen y avisos. |
| `src/opener.rs` | 93 | Abre una ruta con la app predeterminada; no conoce extensiones. |
| `src/scanner.rs` | 234 | Recorrido del disco, ocultos, mtime, tamaño y omisión de rutas no UTF-8. |
| `src/tags.rs` | 372 | Lectura de etiquetas MP3 con lofty y `posix_fadvise(RANDOM)`, solo lectura. |
| `src/video.rs` | 426 | Parser Serie/Temporada/Episodio y variantes. |
| `src/ui/mod.rs` | 14 | Fachada de `ui/`: declara módulos y reexporta `CatalogView` y `view`. |
| `src/ui/root.rs` | 88 | `CatalogView`, estado vacío y despacho de la vista por página. |
| `src/ui/overview.rs` | 58 | Vista “Todo”: resumen y cuadrícula de álbumes. |
| `src/ui/music.rs` | 241 | Vistas de artistas, álbumes y canciones. |
| `src/ui/video.rs` | 466 | Vistas de series, temporadas, episodios y películas. |
| `src/ui/books.rs` | 94 | Vista de libros en tarjetas 2:3 y búsqueda por título/autor. |
| `src/ui/cards.rs` | 393 | Tarjetas, cuadrículas, miniaturas y recorte de títulos; incluye sus tests. |
| `src/ui/common.rs` | 99 | Cabeceras, listas, etiquetas y helpers de búsqueda. |
| `src/ui/status.rs` | 51 | Avisos y estados de escaneo/apertura. |

Los paneles de **detalles** y **alias** no están en `ui/`: viven en `src/app.rs` (`details_content` y `aliases_content`).

Total: 7444 líneas en `src/`. De ellas, **700 líneas de `db.rs` son tests**, **149 de `epub.rs`**, **66 de `opener.rs`**, **35 de `ui/books.rs`** y **22 de `ui/cards.rs`**.

## 3. SQLite

Esquema actual: **versión 4** (`PRAGMA user_version = 4`). `open_at` migra solo si la versión es menor; en aperturas normales no repite el esquema ni las migraciones. Activa `journal_mode=WAL` y `busy_timeout=5000 ms`.

Tabla `files`: `id`, `path UNIQUE`, `name`, `kind`, `creator`, `collection_creator`, `collection`, `title`, `track_number`, `track_total`, `filename_track`, `year`, `cover_hash`, `tag_status`, `mtime`, `size`.

Tabla `artist_aliases`: `alias` (PK `COLLATE NOCASE`), `canonical`; semilla `iamdoechii → Doechii` solo en la primera migración.

Índices: `idx_files_kind(kind)`, `idx_files_collection(collection_creator, collection, year)`.

Migraciones: agrega columnas, renombra `artist→creator`, `album_artist→collection_creator`, `album→collection`, reemplaza el índice viejo. Copia bases/caché desde `com.diegomendoza.Contento` y luego `com.diegomendoza.Catalogo`; no sobrescribe ni borra las anteriores.

## 4. Flujo de un escaneo

1. `init` lanza `db::load_cached` en `spawn_blocking` solo si `catalog_root` no está vacía.
2. Al llegar `CachedLoaded`, se pintan items, biblioteca, alias y video guardados.
3. Si hay raíz, se emite `ScanRequested`: la UI conserva lo guardado y muestra “Escaneando…”. Si la raíz está vacía, no se escanea y se muestra el estado vacío con el selector de carpeta.
4. `db_path` copia la base legacy si la nueva no existe; `open_at` migra solo si hace falta.
5. `readable_root` valida la raíz; si falla, no escribe y devuelve lo guardado con aviso.
6. `scanner::scan` recorre con WalkDir, poda ocultos (`.Trash-1000` incluida), clasifica y guarda mtime/tamaño.
7. `enrich_items` reparte música y EPUB en un pool de máximo 4 hilos. Si mtime/tamaño no cambiaron y `cover_ready`, reutiliza metadata; si no, lee etiquetas o rbook y genera miniatura. Cada archivo va dentro de `catch_unwind`: un panic se convierte en `TagStatus::Error` para ese archivo y no aborta el escaneo. Los PDF se quedan solo con el nombre.
8. Una transacción hace upsert por `path`; borra ausentes solo si el escaneo fue completo, no quedó vacío con filas previas bajo la raíz y las rutas están bajo la raíz.
9. Se recargan filas, se marcan portadas listas y se construye `Library`; la app construye `VideoLibrary`.
10. `ScanFinished` actualiza la UI.

Regla `EmptyRoot`: si el escaneo no encuentra ningún archivo catalogable y la base tiene filas bajo la raíz, se guardan los hallazgos (cero) pero **no se borra nada** y se devuelve `ScanWarning::EmptyRoot`; la UI lo muestra igual que `RootUnavailable`. Limitación: si el usuario vació la carpeta a propósito, las filas se quedan y el aviso se repite hasta que exista la limpieza de faltantes.

## 5. Reglas de AGENTS.md

- Partir del template; verificar APIs de libcosmic antes de usarlas.
- `cargo check` tras cada cambio; al cerrar, `cargo clippy` y pruebas. `just run` lo ejecuta el usuario, no el agente.
- No agregar dependencias sin explicar para qué.
- La app solo lee la colección; miniaturas solo en caché XDG.
- La app no hace peticiones de red. Lee etiquetas y portadas únicamente de los archivos locales.
- lofty solo lectura.
- Cada tipo de medio se define en un solo lugar (extensiones, aspecto, niveles, apertura).
- Widgets/tema de libcosmic, sin colores fijos; iconos simbólicos.
- **MVU** (Model-View-Update): lógica en `update`, UI no bloqueante, SQLite en XDG.
- Rendimiento: releer solo archivos nuevos o con mtime/tamaño cambiados; reutilizar miniaturas; trabajo fuera del hilo UI.

## 6. Decisiones tomadas

- **Solo lectura / sin red:** nunca se escribe en la colección; lofty y rbook se usan solo para leer; no hay clientes HTTP.
- **Portadas:** se detectan barras comparando con el color del borde (tolerancia 12/255). En la inspección inicial, 31/31 tenían barras de 276–280 px e interior 720–728×720. Música 1:1 (512×512); sin barras, `fit`; video/libros 2:3 (512×768). Caché v2; v1 16:9 sin uso. La portada de libro sube ~50 % de píxeles frente a la cuadrada (393,216 vs 262,144); no se optimizó. Todas las fuentes pasan por `covers::ensure_thumbnail`, que rechaza más de 8 MiB o más de 8 MP antes de hashear o decodificar, exige dimensiones mayores que cero y usa `ImageReader` con límites. `local_cover` comprueba el tamaño con `fs::metadata` antes de leer el archivo. La portada incrustada de música ya fue cargada en memoria por lofty; el filtro evita la segunda copia, pero no puede evitar esa carga previa.
- **EPUB:** `rbook 0.7.12` sin features; lectura de título, autores y portada. Portada limitada a 8 MiB y 8 MP; EPUB corrupto o sin metadatos usa el nombre del archivo y `TagStatus::Error`/`Missing`. `catch_unwind` aísla posibles panics. `creator` guarda el autor y `title` el título; sin cambio de esquema.
- **Alias:** tabla editable, sin distinguir mayúsculas; se aplican al agrupar por `collection_creator`.
- **Tipos de medio:** `src/media.rs` es la única fuente de extensiones, aspecto, jerarquía y `OpenAction`.
- **Carpeta del catálogo:** raíz única editable desde “Ver → Carpeta del catálogo…” o desde el estado vacío. Se usa `cosmic::dialog::file_chooser`; se guarda con `write_entry` y el `watch_config` existente dispara el escaneo. Se rechazan las rutas no UTF-8, `/` y la carpeta personal completa con `ScanWarning::InvalidCatalogRoot`. Las rutas no UTF-8 dentro de la colección se omiten y cuentan como error de recorrido, así que el escaneo queda incompleto y no borra filas. Cambiar de raíz no borra filas de la raíz anterior.
- **Sin raíz:** `catalog_root` vacío; no se carga caché, no se escanea y la vista muestra “Elige la carpeta de tu catálogo”.
- **Bug corregido del selector:** con `XDG_CONFIG_HOME` temporal, `watch_config` usa el daemon de configuración por DBus (feature `dbus-config`), que observa la configuración real; el archivo se guardaba en la ruta temporal pero no llegaba `UpdateConfig`. No es un fallo de usuarios nuevos: es un artefacto de probar con una config XDG aislada. Ahora, tras guardar con éxito, la app actualiza la config en memoria y pide el escaneo; un `UpdateConfig` con la misma raíz no lo duplica. Para probar el primer escaneo, cambiar solo `XDG_DATA_HOME` y `XDG_CACHE_HOME`; con `XDG_CONFIG_HOME` temporal el daemon de DBus no ve la config.
- **Apertura:** `media.rs` decide la `OpenAction`; `src/opener.rs` solo ejecuta `open::that_detached(&Path)` en `spawn_blocking`, rechaza rutas relativas y convierte el fallo en un aviso. `LaunchUrl` solo abre URLs que empiezan por `https://` con destino no vacío. El enlace de repositorio del About se omite mientras `CARGO_PKG_REPOSITORY` siga siendo el TODO.
- **Interacción:** un clic en una canción, episodio o libro abre el panel de detalles tras 400 ms; el doble clic abre el archivo sin mostrar ni dejar abierto el panel; el botón Abrir del panel usa el mismo `OpenItem`.
- **Panel de detalles:** se acopla al lado del contenido (`context_is_overlay = false`). La causa del bug era que, como overlay, libcosmic capturaba todos los clics del contenido y el breadcrumb nunca recibía `GoToArtists`; por eso no se podía regresar con el panel abierto. En ventanas angostas el contenido se encoge.
- **Tarjetas:** altura uniforme fija; título 2 líneas con elipsis, creador 1, metadato 1. No se mide la más alta; iced no ofrece altura máxima de fila. Cuadrícula responsive con ancho fijo y espacio repartido; respaldo en `PENDIENTES.md`.
- **Video:** nombre como única fuente; `SxxEyy`; variantes del mismo episodio agrupadas; películas sin patrón aparte; iconos, no pósters.
- **Columnas genéricas:** `creator` y `collection` sustituyen a `artist`/`album` en modelo y SQLite.
- **Optimización:** `cover_ready` evita `fs::metadata` por tarjeta; las claves de búsqueda se precalculan en minúsculas; una sola conexión para alias; pool de miniaturas.
- **Deuda:** `ScanWarning` cubre escaneo, apertura y raíz inválida; el nombre ya no es exacto y renombrarlo queda pendiente.

## 7. Lo que NO está hecho

Copiado de `PENDIENTES.md`:

- Comprobar visualmente la cuadrícula responsive en ventanas muy anchas y muy angostas. Si el reparto de columnas no funciona bien en algún caso, usar el respaldo `FlexRow` con `SpaceBetween` y anotarlo aquí.
- Revisar el recorte de barras con portadas nuevas de fondos uniformes claros.
- Confirmar en pantalla que las dos variantes de `S29E03` se ven bajo el mismo episodio con etiqueta.
- Visores/reproductores propios: no se implementarán; apertura externa.
- Revisar miniaturas viejas v1 (16:9) sin uso.

Además, `PENDIENTES.md` tiene nueve puntos marcados “sin verificar”: portada local por directorio, cambio de `cover.jpg`, TPE2 para agrupar, reintento de etiquetas con error, `rust-version`/README, features de `tokio`, pósters de película, carrera de 8 hilos y filtro/JPEG más rápido.

## 8. Dependencias

| Dependencia | Versión | Uso |
|---|---|---|
| `dirs` | 7 | Rutas XDG de datos/caché. |
| `i18n-embed` | 0.16 | Traducciones Fluent. |
| `i18n-embed-fl` | 0.10 | Macro `fl!`. |
| `image` | 0.25.10 | Decodificar PNG/JPEG y escribir miniaturas. |
| `lofty` | 0.25.5 | Etiquetas y portadas MP3, solo lectura. |
| `rbook` | 0.7.12 (sin features) | Título, autores y portada EPUB, solo lectura. |
| `open` | 5.4.5 | Abrir archivos con la app predeterminada (`that_detached`); también URL del repositorio en About. |
| `rusqlite` | 0.41 (`bundled`) | SQLite embebido. |
| `rust-embed` | 8.8.0 | Traducciones embebidas. |
| `rustix` | 1.1.6 (`fs`, solo Linux) | `posix_fadvise(RANDOM)` al leer etiquetas; sin `unsafe`. |
| `sha2` | 0.11.1 | Hash de portadas. |
| `tokio` | 1.48 (`full`) | `spawn_blocking` y runtime. |
| `walkdir` | 2.5 | Recorrido con poda. |
| `libcosmic` | git master | UI, tema, Application, config. |
| `xdgen` (build) | 0.1 | `.desktop` y metainfo. |

`zip` 9 es solo dev-dependency para generar EPUB de prueba.

Empaquetado: `build.rs` genera `target/xdgen/app.desktop` y `app.metainfo.xml`; `just install` los renombra al ID, instala el icono en `share/icons/hicolor/scalable/apps/com.xoloitzcuintle95.MediatecaLocal.svg` y el metainfo en `share/metainfo/`. `appstreamcli validate --no-net` queda con la alerta `url-homepage-missing` porque la URL del repositorio es un TODO; `desktop-file-validate` pasa.

## 9. Rendimiento antes/después

Mediciones con **base y caché temporales**, release, 292 pistas, 3 videos y 208 portadas únicas. La colección vive en un disco **rotacional** (`/dev/sda1`, `ROTA=1`), así que el costo dominante es el tiempo de búsqueda.

| Escenario | Antes | Después |
|---|---:|---:|
| Primer escaneo, caché vacío | **39.08 s** | **26.46 s** |
| Segundo escaneo | **17.15 ms** | **22.43 ms** |
| Tags leídos / reutilizados (2.º) | 292 / 292 | 292 / 292 |
| Miniaturas v2 generadas | 208 | 208 |

La mejora del primer escaneo fue ~32 %. El segundo es ruido de medición (~ms).

**`fadvise` (solo Linux):** lofty ya leía solo el tag, pero el *readahead* del kernel traía ~5,6× más datos: **1 088 → 195 MiB** con `POSIX_FADV_RANDOM`. Aun así la mejora de pared es moderada porque persisten las búsquedas aleatorias. **Medición fría del usuario con 4 hilos: ~22,6 s contra ~26,5 s originales (~15 %).** El cuello restante es el tiempo de búsqueda del disco. El **progreso de la primera importación sigue pendiente**.

`cargo build --release` incremental: ~28 s. `cargo check`, `cargo clippy --all-targets -- -D warnings` y **80 tests** pasan.

Mediciones del usuario con el binario release y la GUI abierta, **puntuales**:

- CPU en reposo: **0 ticks en 30 s**.
- RSS al abrir: **~209 MB** con 292 pistas y 208 portadas.
- RSS tras navegar: **~208 MB**. Sin crecimiento observado.
- Navegación probada: música → artistas → álbumes → canciones → volver; después videos; después libros; después otra vez música.
- Referencia aproximada: COSMIC Files usa **~148 MB**.
- Es **una sola sesión** de prueba: una muestra tras navegar, sin crecimiento observado; no prueba formal.

La medición anterior de **4.40 s** (200 pistas, 31 lecturas, 144 miniaturas) **no era un primer escaneo**: fue un escaneo incremental sobre la base y el caché reales, con solo 31 archivos nuevos/cambiados y 144 miniaturas escritas. No es comparable con las cifras de arriba. Al medir hoy: 292 pistas, 3 videos, 0 libros y 208 portadas únicas; el número real puede cambiar si el disco cambia.

## Verificado

- Aviso de fallo al abrir sin lanzadores disponibles: apareció y la app no se congeló.
- `xdg-open` real abrió un archivo con un nombre raro.

## No verificado

- Tiempo desde abrir la GUI hasta ver contenido: no medido. CPU en reposo y RSS (al abrir y tras navegar) ya fueron medidos por el usuario; ver §9.
- Apariencia real/responsive y variantes de `S29E03` en pantalla.
- Selector de carpeta, portal XDG y estado vacío en la GUI.
- Escritura real en cosmic-config desde el selector (la transición en memoria sí está probada).
- Clic real en el breadcrumb con el panel de detalles acoplado (solo verificado en código y con prueba de `update`).
- EPUB reales con estructuras raras y los límites de portada en la GUI.
- Icono renderizado en el escritorio real y la URL definitiva del repositorio.
- Migración de configuración entre IDs (solo base y caché).
