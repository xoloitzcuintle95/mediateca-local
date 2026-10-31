# Traspaso — Mediateca Local

Fecha: 2026-10-31  
HEAD: `9f20531 Limpieza: sin metadatos en línea, icono, metainfo y repositorio`  
Árbol de trabajo: limpio antes de crear este archivo.

## Nota sobre “Tanda 1”

El repo ya no conserva la división por tandas en `PENDIENTES.md`. La definición histórica está en el commit `d5de2c1`: la Tanda 1 era **tarjetas uniformes + portadas cuadradas v2**, con dos verificaciones visuales pendientes.

Interpretando “Tanda 1” como ese primer bloque:

### Completado

- Tarjetas con altura uniforme, título a dos líneas y metadatos a una línea.
- Portadas cuadradas v2 para música, con caché por contenido.
- Proporciones 2:3 para video y libros.
- Cuadrícula responsive con ancho de tarjeta fijo.
- Recorte/encaje de portadas y detección de barras.

### Pendiente de esa tanda

- Verificar visualmente la cuadrícula en ventanas muy anchas y muy angostas.
- Revisar el recorte de barras con portadas nuevas de fondos uniformes claros.

## Trabajo completado después de la Tanda 1

- Catálogo local de música, video y libros con SQLite v4.
- Escaneo incremental por `mtime`/`tamaño`, reutilización de metadata y miniaturas.
- Borrado seguro de filas ausentes y guardarraíl `EmptyRoot`.
- Agrupación musical artista → álbum → canción y alias de artista.
- Parser de video Serie/Temporada/Episodio, variantes y películas.
- Metadatos y portadas EPUB con `rbook`, solo lectura, con límites de tamaño y píxeles.
- Portadas de libro 2:3 en tarjetas y en el panel de detalles.
- Búsqueda de libros por nombre, título y autor.
- Apertura con `open::that_detached`, sin bloquear la UI.
- Selector de carpeta del catálogo con cosmic-config; raíz vacía muestra estado vacío.
- Corrección del selector cuando `XDG_CONFIG_HOME` es temporal.
- Interacción: un clic abre detalles tras 400 ms; doble clic abre el archivo.
- Panel de detalles acoplado para que el contenido siga recibiendo clics.
- División de `ui/mod.rs` en módulos por responsabilidad.
- Icono SVG provisional, metainfo corregido y empaquetado en `~/.local`.
- Icono del lanzador verificado tras reiniciar sesión: ya se ve correctamente.
- `open` alineado a 5.4.5; sin cliente HTTP ni metadatos en línea.
- 65 tests; clippy con `-D warnings` limpio en la última ejecución.

## Archivos principales

### Código Rust

- `src/main.rs`: arranque, i18n y `cosmic::app::run`.
- `src/app.rs`: modelo, mensajes, rutas, escaneo, selector de carpeta, apertura y paneles.
- `src/config.rs`: `catalog_root` en cosmic-config, default vacío.
- `src/db.rs`: SQLite, migraciones, upsert, borrado seguro, alias y enrich.
- `src/scanner.rs`: recorrido del disco.
- `src/tags.rs`: etiquetas MP3 con lofty, solo lectura.
- `src/epub.rs`: metadatos y portada EPUB con rbook, solo lectura.
- `src/covers.rs`: miniaturas JPEG v2 y caché.
- `src/library.rs`: agrupación musical.
- `src/video.rs`: parser de video.
- `src/media.rs`: tabla única de tipos de medio.
- `src/model.rs`: tipos compartidos y `APP_ID`.
- `src/opener.rs`: apertura externa.
- `src/i18n.rs`: Fluent es/en.
- `src/ui/`: fachada, raíz, overview, música, video, libros, tarjetas, helpers y estados.

### Configuración, empaquetado y docs

- `Cargo.toml`, `Cargo.lock`: dependencias; `open 5.4.5`, `rbook 0.7.12`, `zip` solo en tests.
- `justfile`: instalación en `~/.local`, `DESTDIR`, icono y metainfo.
- `resources/app.desktop`
- `resources/app.metainfo.xml`
- `resources/icons/hicolor/scalable/apps/icon.svg`
- `i18n/es/mediateca_local.ftl`
- `i18n/en/mediateca_local.ftl`
- `AGENTS.md`
- `docs/ESTADO.md`
- `PENDIENTES.md`

## Qué falta

- Poner la URL real del repositorio: `Cargo.toml` y el metainfo tienen un TODO.
- Resolver la alerta de AppStream `url-homepage-missing` cuando exista la URL.
- Verificar visualmente la cuadrícula responsive y el recorte de barras con fondos claros.
- Confirmar en pantalla las dos variantes de `S29E03`.
- Investigar por qué el pool de 4 hilos solo mejoró el primer escaneo ~32 %.
- Mostrar progreso y llenar la lista durante la primera importación.
- Botón para borrar base y caché con aviso, para probar el primer escaneo.
- Revisar/limpiar miniaturas viejas v1 (16:9).
- Deuda: `ScanWarning` cubre escaneo, apertura y raíz inválida; renombrarlo queda pendiente.
- Sin verificar: selector de carpeta y escritura real en cosmic-config desde la GUI, EPUB reales con estructuras raras, límites de portada con archivos grandes, memoria real con varias portadas 2:3.

## Estado de compilación

```text
cargo check --locked -> OK
```

Última tanda conocida: `cargo clippy --all-targets -- -D warnings` limpio y **65 tests** pasando. No se ejecutó `just run` durante el trabajo del agente.

## Aviso de documentación

`docs/ESTADO.md` tiene el encabezado en `4c28ad1`, pero el HEAD actual es `9f20531`. El contenido del documento es correcto salvo ese hash.
