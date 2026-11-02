app-title = Mediateca Local
app-comment = Un catálogo local para tu música, videos y libros
app-keywords = música;video;libros;catálogo
about = Acerca de
repository = Repositorio
view = Ver

nav-all = Todo
nav-music = Música
nav-video = Video
nav-books = Libros
artists = Artistas
albums = Álbumes
tracks = Canciones
series = Series
seasons = Temporadas
episodes = Episodios
authors = Autores
sagas = Sagas
books = Libros
movies = Películas

rescan = Volver a escanear
scanning = Escaneando…
empty-music = No se encontró música
empty-tracks = No se encontraron canciones
empty-generic = No se encontraron archivos
deleted = { $count ->
    [one] { $count } entrada eliminada
   *[other] { $count } entradas eliminadas
}

summary = { $tracks } canciones · { $albums } álbumes · { $artists } artistas · { $videos } videos · { $books } libros
artists-count = { $count } artistas
albums-count = { $count } álbumes
tracks-count = { $count } canciones
videos-count = { $count } videos
books-count = { $count } libros

single = Sencillo
single-with-year = Sencillo · { $year }
album-subtitle = { $count } canciones · { $year }
album-no-year = { $count } canciones

unknown-artist = Artista desconocido
unknown-author = Autor desconocido
unknown-album = Sin álbum
unknown-series = Serie desconocida
unknown-year = Sin año
kind-video = Video
kind-book = Libro
kind-music = Música

series-count = { $count } series
seasons-count = { $count } temporadas
episodes-count = { $count } episodios
movies-count = { $count } películas
series-subtitle = { $seasons } temporadas · { $episodes } episodios
video-summary = { $series } series · { $movies } películas
season-label = Temporada { $number }
episode-label = Episodio { $number }
video-details-subtitle = { $series } · { $season } · { $variant }
movie = Película
file = Archivo

search-placeholder = Buscar artista, álbum o título…
details = Detalles
details-empty = Selecciona un elemento para ver sus detalles.
details-subtitle = { $artist } · { $album }
path = Ruta
open = Abrir
catalog-folder = Carpeta del catálogo…
choose-catalog-folder = Elige la carpeta de tu catálogo
choose-folder = Elegir carpeta…
catalog-folder-not-utf8 = La ruta elegida no es UTF-8 válido.
catalog-folder-invalid = La carpeta elegida no se pudo interpretar.
catalog-folder-portal = Esa carpeta no es accesible directamente. Elige una carpeta real desde tu carpeta personal u otra unidad montada.
catalog-folder-too-broad = Esa carpeta es demasiado amplia para el catálogo. Elige una carpeta dentro de tu carpeta personal.

aliases = Alias de artistas
aliases-help = Los alias unen variantes del nombre de un artista. No distinguen mayúsculas.
aliases-empty = No hay alias configurados.
alias-original = Variante
alias-original-placeholder = ej. iamdoechii
alias-canonical = Nombre canónico
alias-canonical-placeholder = ej. Doechii
add-alias = Agregar alias
remove = Quitar
alias-empty = Los dos nombres son obligatorios.

scan-failed = No se pudo actualizar el catálogo: { $reason }
warning-root-unavailable = No se pudo leer { $root }: { $reason }. Se muestra el catálogo guardado; no se eliminó nada.
warning-incomplete = El escaneo quedó incompleto ({ $errors } errores). No se eliminaron entradas del catálogo.
warning-empty-root = La carpeta está vacía; puede que el disco no esté conectado. No se borró nada del catálogo.
warning-invalid-root = No se pudo usar esa carpeta: { $reason }
warning-open-failed = No se pudo abrir el archivo: { $reason }

git-description = Git commit {$hash} del {$date}
