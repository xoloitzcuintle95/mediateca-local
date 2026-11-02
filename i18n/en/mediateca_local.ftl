app-title = Mediateca Local
app-comment = A local media catalog for your music, videos and books
app-keywords = music;video;books;catalog
about = About
repository = Repository
view = View

nav-all = All
nav-music = Music
nav-video = Video
nav-books = Books
artists = Artists
albums = Albums
tracks = Tracks
series = Series
seasons = Seasons
episodes = Episodes
authors = Authors
sagas = Sagas
books = Books
movies = Movies

rescan = Rescan
scanning = Scanning…
empty-music = No music found
empty-tracks = No tracks found
empty-generic = No files found
deleted = { $count ->
    [one] { $count } removed entry
   *[other] { $count } removed entries
}

summary = { $tracks } tracks · { $albums } albums · { $artists } artists · { $videos } videos · { $books } books
artists-count = { $count } artists
albums-count = { $count } albums
tracks-count = { $count } tracks
videos-count = { $count } videos
books-count = { $count } books

single = Single
single-with-year = Single · { $year }
album-subtitle = { $count } tracks · { $year }
album-no-year = { $count } tracks

unknown-artist = Unknown artist
unknown-author = Unknown author
unknown-album = Unknown album
unknown-series = Unknown series
unknown-year = Unknown year
kind-video = Video
kind-book = Book
kind-music = Music

series-count = { $count } series
seasons-count = { $count } seasons
episodes-count = { $count } episodes
movies-count = { $count } movies
series-subtitle = { $seasons } seasons · { $episodes } episodes
video-summary = { $series } series · { $movies } movies
season-label = Season { $number }
episode-label = Episode { $number }
video-details-subtitle = { $series } · { $season } · { $variant }
movie = Movie
file = File

search-placeholder = Search artist, album or title…
details = Details
details-empty = Select an item to see its details.
details-subtitle = { $artist } · { $album }
path = Path
open = Open
catalog-folder = Catalog folder…
choose-catalog-folder = Choose your catalog folder
choose-folder = Choose folder…
catalog-folder-not-utf8 = The selected path is not valid UTF-8.
catalog-folder-invalid = The selected folder could not be interpreted.
catalog-folder-portal = That folder is not directly accessible. Choose a real folder from your home directory or another mounted drive.
catalog-folder-too-broad = That folder is too broad for the catalog. Choose a folder inside your home directory.

aliases = Artist aliases
aliases-help = Aliases merge artist name variants. Matching ignores case.
aliases-empty = No aliases configured.
alias-original = Variant
alias-original-placeholder = e.g. iamdoechii
alias-canonical = Canonical name
alias-canonical-placeholder = e.g. Doechii
add-alias = Add alias
remove = Remove
alias-empty = Both names are required.

scan-failed = Could not update the catalog: { $reason }
warning-root-unavailable = Could not read { $root }: { $reason }. Showing the saved catalog; nothing was deleted.
warning-incomplete = The scan was incomplete ({ $errors } errors). No catalog entries were deleted.
warning-empty-root = The folder is empty; the drive may not be connected. Nothing was removed from the catalog.
warning-invalid-root = Could not use that folder: { $reason }
warning-open-failed = Could not open the file: { $reason }

git-description = Git commit {$hash} on {$date}
