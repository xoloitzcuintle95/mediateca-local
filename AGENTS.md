# Mediateca Local — catálogo multimedia para COSMIC

App de escritorio para COSMIC (Pop!_OS 24.04) en Rust con libcosmic. Cataloga archivos locales (video, música, libros) y los abre con la app predeterminada del sistema. El usuario es principiante: explica en español sencillo y haz cambios pequeños.

## Reglas de trabajo
- Parte SIEMPRE del código del template. Antes de usar cualquier API de libcosmic, búscala en el código del template, en los ejemplos de libcosmic (descargados en ~/.cargo/git/checkouts/) o con `cargo doc`. No inventes APIs de memoria: libcosmic cambia seguido.
- Después de cada cambio corre `cargo check`. Al cerrar una tarea, corre `cargo clippy` y las pruebas; `just run` lo ejecuta el usuario, no el agente. Nunca digas "listo" si no compila.
- Si fallas 3 veces con el mismo error, detente y explícame el problema.
- No agregues dependencias sin decirme para qué.
- La app solo LEE las carpetas del catálogo: nunca borra, mueve ni modifica archivos del usuario. Las portadas y miniaturas se guardan solo en el directorio de caché del usuario (XDG_CACHE_HOME), nunca dentro de la colección.
- La app no hace peticiones de red. Lee etiquetas y portadas únicamente de los archivos locales.
- Las etiquetas se leen con lofty en modo solo lectura; no se usan sus APIs de escritura.

## Tipos de medio
- Cada tipo de medio se define en un solo lugar: extensiones, proporción de portada, nombres de sus niveles de jerarquía y forma de abrir el archivo.

## Diseño (debe verse como app oficial de COSMIC)
- Usa widgets de libcosmic, no widgets crudos de iced cuando exista equivalente.
- Estructura estándar de COSMIC: barra de encabezado con buscador, barra de navegación lateral (Todo / Música / Video / Libros) y panel de contexto para detalles y ajustes.
- Colores, espaciados y radios salen del tema (cosmic-theme). Cero colores fijos. Debe verse bien en modo claro y oscuro.
- Iconos simbólicos del tema del sistema, no emojis.
- Los ajustes se guardan con cosmic-config.

## Arquitectura
- ID de la app: `com.xoloitzcuintle95.MediatecaLocal`, definido en `src/model.rs`.
- Patrón Model-View-Update: Message -> update -> view. Nada de lógica pesada en view.
- Escaneo y base de datos en tareas asíncronas: la UI nunca se congela.
- SQLite en el directorio de datos del usuario (XDG), no dentro del proyecto.
- Abrir archivos con `xdg-open`, como proceso separado de la app.
- Módulos: model, db, scanner, opener, ui/.

## Rendimiento
- Re-escanear solo relee etiquetas y portadas de archivos nuevos o cuyo tamaño o mtime cambiaron. Para los demás se reutiliza la metadata y el hash de portada guardados en SQLite.
- El escaneo, la lectura de etiquetas y la generación de miniaturas corren fuera del hilo de la UI.
- Una portada se procesa una sola vez por contenido; las siguientes peticiones reutilizan la miniatura del caché.

## Comandos
- `cargo check`, `cargo clippy`, `just build-release`; `just run` lo ejecuta el usuario.
