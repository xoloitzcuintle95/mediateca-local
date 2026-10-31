# Pendientes

## Verificación pendiente
- Comprobar visualmente la cuadrícula responsive en ventanas muy anchas y muy angostas. Si el reparto de columnas no funciona bien en algún caso, usar el respaldo `FlexRow` con `SpaceBetween` y anotarlo aquí.
- Revisar el recorte de barras con portadas nuevas que tengan fondos uniformes claros.
- Confirmar en pantalla que las dos variantes de `S29E03` se ven bajo el mismo episodio con su etiqueta.

## Optimización
- Investigar por qué los 4 hilos solo mejoraron el primer escaneo un 32 %.

## Después
- Botón para borrar todo el catálogo (base y caché) con aviso, para probar el primer escaneo; la carpeta de la colección nunca se toca.
- Primera importación (~26 s con la base vacía, 292 pistas y 208 portadas): mostrar progreso y llenar la lista mientras avanza, en vez de esperar al final.
- Visores/reproductores propios: no se implementarán; la apertura será externa.
- Revisar miniaturas antiguas v1 (16:9) que quedan en caché sin uso.

## Sin verificar
- Leer `cover.jpg` y hashearlo una sola vez por directorio, no una vez por pista (sin verificar).
- Detectar el cambio de `cover.jpg` aunque el archivo de audio no cambie de tamaño ni de fecha (sin verificar).
- Usar el artista del álbum (TPE2) para agrupar, en vez del primer artista de la pista (sin verificar).
- Reintentar la lectura de etiquetas cuando quedó en error, sin esperar a que cambie el archivo (sin verificar).
- Declarar `rust-version` en `Cargo.toml` y documentar el requisito de Rust en el README (sin verificar).
- Reducir las features de `tokio` a las que realmente se usan (sin verificar).
- Pósters de película: leer una imagen con el mismo nombre del video junto al archivo (opcional y local); las series necesitarían carpeta por serie; hoy todo video usa icono (sin verificar).
- Con 8 hilos, dos hilos pueden decodificar la misma portada a la vez (carrera); falta deduplicar por hash en vuelo (sin verificar).
- Probar un filtro de reescalado más rápido que Lanczos3 o miniaturas más chicas (por ejemplo, 384 px) para bajar render/JPEG; comparar antes la calidad visual (sin verificar).
