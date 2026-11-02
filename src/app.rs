// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::covers;
use crate::db::{self, CachedCatalog};
use crate::fl;
use crate::library::{AlbumKey, AliasMap, Library};
use crate::model::{Kind, MediaItem, OpenAction, ScanOutcome, ScanWarning};
use crate::opener;
use crate::ui::{self, CatalogView};
use crate::video::VideoLibrary;
use cosmic::Application;
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::{Length, Subscription, Task};
use cosmic::prelude::*;
use cosmic::widget::{self, about::About, icon, menu, nav_bar};

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// The about page for this app.
    about: About,
    /// Contains items assigned to the nav bar panel.
    nav: nav_bar::Model,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    /// Configuration data that persists between application runs.
    config: Config,
    /// All files, used for video and book lists.
    items: Vec<MediaItem>,
    /// Music grouped by artist and album.
    library: Library,
    /// Current artist aliases.
    aliases: AliasMap,
    /// Video grouped by series, season and episode.
    video_library: VideoLibrary,
    /// Route inside the video library.
    video_route: VideoRoute,
    /// Current state of the background scan.
    scan_state: ScanState,
    /// Rows removed by the last complete scan.
    deleted: usize,
    /// Non-fatal warning produced by the last scan.
    warning: Option<ScanWarning>,
    /// Current header search text.
    search: String,
    /// Route inside the music library.
    music_route: MusicRoute,
    /// Album opened from the "Todo" overview.
    overview_album: Option<AlbumKey>,
    /// Details shown in the context drawer.
    details: Option<Details>,
    /// Primer clic pendiente de confirmar como clic simple.
    pending_click: Option<PendingClick>,
    /// Generación del clic pendiente; descarta temporizadores viejos.
    click_seq: u64,
    /// New alias being edited.
    alias_input: String,
    /// Canonical artist for the new alias.
    canonical_input: String,
    /// Error from the last alias operation.
    alias_error: Option<String>,
    /// Thumbnail cache directory.
    cache_dir: PathBuf,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    CachedLoaded(Result<CachedCatalog, String>),
    ScanRequested,
    ScanFinished(Result<ScanOutcome, String>),
    DismissWarning,
    DismissScanError,
    SearchChanged(String),
    OpenArtist(String),
    OpenAlbum(AlbumKey),
    GoToArtists,
    GoToOverview,
    OpenSeries(String),
    OpenSeason {
        series: String,
        season: u32,
    },
    GoToSeries,
    GoToSeasons(String),
    ChooseCatalogFolder,
    CatalogFolderChosen(Result<Option<PathBuf>, String>),
    /// Primer clic en un elemento; espera a ver si llega un doble clic.
    ItemClicked(Details),
    /// Confirma el clic simple si no llegó un segundo clic a tiempo.
    ClickTimeout(u64),
    /// Abre el archivo; no muestra ni cierra el panel de detalles.
    OpenItem(Details),
    OpenFinished(Result<(), String>),
    AliasChanged(String),
    CanonicalChanged(String),
    SaveAlias,
    DeleteAlias(String),
    AliasesSaved(Result<AliasMap, String>),
}

/// Datos que se muestran en el panel de detalles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Details {
    /// Identificador del archivo, para emparejar los dos clics.
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub path: String,
    pub cover_hash: Option<String>,
    pub cover_ready: bool,
    /// Tipo de medio, para elegir la proporción de la portada.
    pub kind: Kind,
    /// Acción decidida por el tipo de medio en `media.rs`.
    pub open_action: OpenAction,
}

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

/// Primer clic guardado mientras se espera a un posible doble clic.
struct PendingClick {
    id: i64,
    at: Instant,
    seq: u64,
    details: Details,
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = db::APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        mut core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // El panel de detalles se acopla al contenido; si es overlay, libcosmic
        // captura todos los clics del contenido y el breadcrumb no responde.
        core.window.context_is_overlay = false;

        let mut nav = nav_bar::Model::default();

        nav.insert()
            .text(Page::All.label())
            .data::<Page>(Page::All)
            .icon(icon::from_name("folder-symbolic"))
            .activate();

        for (page, icon_name) in [
            (Page::Music, "folder-music-symbolic"),
            (Page::Video, "folder-videos-symbolic"),
            (Page::Books, "folder-documents-symbolic"),
        ] {
            nav.insert()
                .text(page.label())
                .data::<Page>(page)
                .icon(icon::from_name(icon_name));
        }

        // Create the about widget. El enlace solo se muestra si la URL del
        // repositorio es real y usa https; mientras siga el TODO, se omite.
        let about = About::default()
            .name(fl!("app-title"))
            .icon(widget::icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .license(env!("CARGO_PKG_LICENSE"));

        let about = match repository_url() {
            Some(url) => about.links([(fl!("repository"), url)]),
            None => about,
        };

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            about,
            nav,
            key_binds: HashMap::new(),
            // Optional configuration file for an application.
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => config,
                })
                .unwrap_or_default(),
            items: Vec::new(),
            library: Library::default(),
            aliases: AliasMap::new(),
            video_library: VideoLibrary::default(),
            video_route: VideoRoute::default(),
            scan_state: ScanState::Idle,
            deleted: 0,
            warning: None,
            search: String::new(),
            music_route: MusicRoute::default(),
            overview_album: None,
            details: None,
            pending_click: None,
            click_seq: 0,
            alias_input: String::new(),
            canonical_input: String::new(),
            alias_error: None,
            cache_dir: covers::cache_dir().unwrap_or_default(),
        };

        // Muestra de inmediato lo guardado; el escaneo arranca después.
        let command = if app.config.catalog_root.trim().is_empty() {
            app.update_title()
        } else {
            Task::batch([
                app.update_title(),
                Task::perform(load_cached(), Message::CachedLoaded).map(cosmic::Action::App),
            ])
        };

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let scanning = matches!(self.scan_state, ScanState::Running);
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Entry(
                        menu::Entry::new(fl!("catalog-folder"), MenuAction::CatalogFolder)
                            .enabled(!scanning),
                    ),
                    menu::Item::Button(fl!("aliases"), None, MenuAction::Aliases),
                    menu::Item::Button(fl!("about"), None, MenuAction::About),
                ],
            ),
        )]);

        vec![menu_bar.into()]
    }

    /// Search field in the center of the header bar.
    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        vec![
            widget::text_input::search_input(fl!("search-placeholder"), &self.search)
                .on_input(Message::SearchChanged)
                .width(Length::Fixed(360.0))
                .into(),
        ]
    }

    /// The rescan button lives at the end of the header bar.
    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        let scanning = matches!(self.scan_state, ScanState::Running);
        let button = widget::button::icon(icon::from_name("view-refresh-symbolic").size(16))
            .tooltip(fl!("rescan"))
            .on_press_maybe((!scanning).then_some(Message::ScanRequested));

        vec![button.into()]
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::about(
                &self.about,
                |url| Message::LaunchUrl(url.to_string()),
                Message::ToggleContextPage(ContextPage::About),
            ),
            ContextPage::Aliases => context_drawer::context_drawer(
                self.aliases_content(),
                Message::ToggleContextPage(ContextPage::Aliases),
            )
            .title(fl!("aliases")),
            ContextPage::Details => context_drawer::context_drawer(
                self.details_content(),
                Message::ToggleContextPage(ContextPage::Details),
            )
            .title(fl!("details")),
        })
    }

    /// Describes the interface based on the current state of the application model.
    fn view(&self) -> Element<'_, Self::Message> {
        let page = self.nav.active_data::<Page>().copied().unwrap_or_default();

        ui::view(CatalogView {
            page,
            music_route: &self.music_route,
            overview_album: self.overview_album.as_ref(),
            library: &self.library,
            video_library: &self.video_library,
            video_route: &self.video_route,
            items: &self.items,
            search: &self.search,
            scan_state: &self.scan_state,
            deleted: self.deleted,
            warning: self.warning.as_ref(),
            cache_dir: &self.cache_dir,
            catalog_root: &self.config.catalog_root,
        })
    }

    /// Register subscriptions for this application.
    fn subscription(&self) -> Subscription<Self::Message> {
        self.core()
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::UpdateConfig(update.config))
    }

    /// Handles messages emitted by the application and its widgets.
    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::CachedLoaded(result) => {
                match result {
                    Ok(cached) => {
                        self.items = cached.items;
                        self.library = cached.library;
                        self.aliases = cached.aliases;
                        self.video_library = VideoLibrary::build(&self.items);
                        self.validate_route();
                    }
                    Err(error) => {
                        eprintln!("no se pudo cargar el catálogo guardado: {error}");
                    }
                }

                if self.config.catalog_root.trim().is_empty() {
                    return Task::none();
                }

                self.update(Message::ScanRequested)
            }

            Message::ScanRequested => {
                if self.config.catalog_root.trim().is_empty() {
                    self.scan_state = ScanState::Idle;
                    return Task::none();
                }

                if self.scan_state == ScanState::Running {
                    return Task::none();
                }

                self.scan_state = ScanState::Running;
                self.deleted = 0;
                self.warning = None;

                let root = PathBuf::from(&self.config.catalog_root);

                Task::perform(
                    async move {
                        match tokio::task::spawn_blocking(move || db::sync(&root)).await {
                            Ok(result) => result,
                            Err(error) => Err(format!("No se pudo completar el escaneo: {error}")),
                        }
                    },
                    Message::ScanFinished,
                )
                .map(cosmic::Action::App)
            }

            Message::ScanFinished(result) => {
                match result {
                    Ok(outcome) => {
                        self.items = outcome.items;
                        self.library = outcome.library;
                        self.aliases = outcome.aliases;
                        self.video_library = VideoLibrary::build(&self.items);
                        self.deleted = outcome.deleted;
                        self.warning = outcome.warning;
                        self.scan_state = ScanState::Idle;
                        self.validate_route();
                    }
                    Err(error) => {
                        self.scan_state = ScanState::Failed(error);
                    }
                }

                Task::none()
            }

            Message::DismissWarning => {
                self.warning = None;
                Task::none()
            }

            Message::DismissScanError => {
                self.scan_state = ScanState::Idle;
                Task::none()
            }

            Message::SearchChanged(search) => {
                self.search = search;
                Task::none()
            }

            Message::OpenArtist(artist) => {
                self.close_details();
                self.music_route = MusicRoute::Albums { artist };
                Task::none()
            }

            Message::OpenAlbum(key) => {
                self.close_details();

                if matches!(self.nav.active_data::<Page>(), Some(Page::All)) {
                    self.overview_album = Some(key);
                } else {
                    self.music_route = MusicRoute::Album { key };
                }
                Task::none()
            }

            Message::GoToArtists => {
                self.close_details();
                self.music_route = MusicRoute::Artists;
                Task::none()
            }

            Message::GoToOverview => {
                self.close_details();
                self.overview_album = None;
                Task::none()
            }

            Message::OpenSeries(series) | Message::GoToSeasons(series) => {
                self.close_details();
                self.video_route = VideoRoute::Seasons { series };
                Task::none()
            }

            Message::OpenSeason { series, season } => {
                self.close_details();
                self.video_route = VideoRoute::Episodes { series, season };
                Task::none()
            }

            Message::GoToSeries => {
                self.close_details();
                self.video_route = VideoRoute::Series;
                Task::none()
            }

            Message::ChooseCatalogFolder => {
                Task::perform(choose_catalog_folder(), Message::CatalogFolderChosen)
                    .map(cosmic::Action::App)
            }

            Message::CatalogFolderChosen(result) => match result {
                Ok(None) => Task::none(),
                Ok(Some(path)) => self.set_catalog_root(&path),
                Err(reason) => {
                    self.warning = Some(ScanWarning::InvalidCatalogRoot { reason });
                    Task::none()
                }
            },

            Message::ItemClicked(details) => {
                let now = Instant::now();
                let is_double_click = self.pending_click.as_ref().is_some_and(|pending| {
                    pending.id == details.id
                        && now.duration_since(pending.at) <= DOUBLE_CLICK_WINDOW
                });

                if is_double_click {
                    self.pending_click = None;
                    self.close_details();
                    return self.update(Message::OpenItem(details));
                }

                self.click_seq = self.click_seq.wrapping_add(1);
                let seq = self.click_seq;
                let id = details.id;
                self.pending_click = Some(PendingClick {
                    id,
                    at: now,
                    seq,
                    details,
                });

                Task::perform(
                    async move { tokio::time::sleep(DOUBLE_CLICK_WINDOW).await },
                    move |()| Message::ClickTimeout(seq),
                )
                .map(cosmic::Action::App)
            }

            Message::ClickTimeout(seq) => {
                if self
                    .pending_click
                    .as_ref()
                    .is_some_and(|pending| pending.seq == seq)
                    && let Some(pending) = self.pending_click.take()
                {
                    self.details = Some(pending.details);
                    self.context_page = ContextPage::Details;
                    self.core.set_show_context(true);
                }

                Task::none()
            }

            Message::OpenItem(details) => {
                let path = PathBuf::from(&details.path);
                let action = details.open_action;

                match action {
                    OpenAction::XdgOpen => Task::perform(open_file(path), Message::OpenFinished)
                        .map(cosmic::Action::App),
                }
            }

            Message::OpenFinished(result) => {
                if let Err(reason) = result {
                    self.warning = Some(ScanWarning::OpenFailed { reason });
                }

                Task::none()
            }

            Message::AliasChanged(alias) => {
                self.alias_input = alias;
                Task::none()
            }

            Message::CanonicalChanged(canonical) => {
                self.canonical_input = canonical;
                Task::none()
            }

            Message::SaveAlias => {
                let alias = self.alias_input.trim().to_owned();
                let canonical = self.canonical_input.trim().to_owned();

                if alias.is_empty() || canonical.is_empty() {
                    self.alias_error = Some(fl!("alias-empty"));
                    return Task::none();
                }

                Task::perform(save_alias(alias, canonical), Message::AliasesSaved)
                    .map(cosmic::Action::App)
            }

            Message::DeleteAlias(alias) => {
                Task::perform(delete_alias(alias), Message::AliasesSaved).map(cosmic::Action::App)
            }

            Message::AliasesSaved(result) => {
                match result {
                    Ok(aliases) => {
                        self.aliases = aliases;
                        self.alias_input.clear();
                        self.canonical_input.clear();
                        self.alias_error = None;
                        self.rebuild_library();
                    }
                    Err(error) => {
                        self.alias_error = Some(error);
                    }
                }

                Task::none()
            }

            Message::ToggleContextPage(context_page) => {
                let show = if self.context_page == context_page {
                    // Close the context drawer if the toggled context page is the same.
                    !self.core.window.show_context
                } else {
                    // Open the context drawer to display the requested context page.
                    self.context_page = context_page;
                    true
                };

                self.core.set_show_context(show);
                Task::none()
            }

            Message::UpdateConfig(config) => {
                let root_changed = config.catalog_root != self.config.catalog_root;
                self.config = config;

                if root_changed {
                    self.close_details();

                    if self.config.catalog_root.trim().is_empty() {
                        self.clear_catalog();
                        Task::none()
                    } else {
                        self.update(Message::ScanRequested)
                    }
                } else {
                    Task::none()
                }
            }

            Message::LaunchUrl(url) => {
                if !is_https_url(&url) {
                    eprintln!("se ignoró una URL que no es https: {url:?}");
                    return Task::none();
                }

                match open::that_detached(&url) {
                    Ok(()) => {}
                    Err(error) => {
                        eprintln!("failed to open {url:?}: {error}");
                    }
                }

                Task::none()
            }
        }
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.close_details();
        self.nav.activate(id);

        match self.nav.active_data::<Page>().copied() {
            Some(Page::Music) => {
                self.music_route = MusicRoute::Artists;
                self.overview_album = None;
            }
            Some(Page::All) => {
                self.overview_album = None;
            }
            Some(Page::Video) => {
                self.video_route = VideoRoute::Series;
            }
            _ => {}
        }

        self.update_title()
    }
}

impl AppModel {
    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }

    fn rebuild_library(&mut self) {
        self.library = Library::build(&self.items, &self.aliases);
        self.validate_route();
    }

    /// Cierra el panel de detalles y descarta cualquier clic pendiente.
    fn close_details(&mut self) {
        self.pending_click = None;
        self.details = None;

        if self.context_page == ContextPage::Details {
            self.core.set_show_context(false);
        }
    }

    /// Guarda la raíz elegida; el watch de configuración pide el escaneo.
    fn set_catalog_root(&mut self, path: &Path) -> Task<cosmic::Action<Message>> {
        let root = match catalog_root_from_path(path) {
            Ok(root) => root,
            Err(reason) => {
                self.warning = Some(ScanWarning::InvalidCatalogRoot { reason });
                return Task::none();
            }
        };

        let mut config = self.config.clone();
        config.catalog_root = root;

        let context = match cosmic_config::Config::new(db::APP_ID, Config::VERSION) {
            Ok(context) => context,
            Err(error) => {
                self.warning = Some(ScanWarning::InvalidCatalogRoot {
                    reason: error.to_string(),
                });
                return Task::none();
            }
        };

        if let Err(error) = config.write_entry(&context) {
            self.warning = Some(ScanWarning::InvalidCatalogRoot {
                reason: error.to_string(),
            });
            return Task::none();
        }

        self.apply_catalog_root(config)
    }

    /// Aplica una raíz ya guardada y pide el escaneo sin esperar al observador.
    fn apply_catalog_root(&mut self, config: Config) -> Task<cosmic::Action<Message>> {
        self.config = config;
        self.close_details();
        self.warning = None;
        self.update(Message::ScanRequested)
    }

    /// Deja la app en el estado vacío cuando no hay raíz configurada.
    fn clear_catalog(&mut self) {
        self.items.clear();
        self.library = Library::default();
        self.aliases = AliasMap::new();
        self.video_library = VideoLibrary::default();
        self.deleted = 0;
        self.warning = None;
        self.scan_state = ScanState::Idle;
        self.music_route = MusicRoute::default();
        self.video_route = VideoRoute::default();
        self.overview_album = None;
    }

    fn validate_route(&mut self) {
        if let MusicRoute::Albums { artist } = &self.music_route
            && !self
                .library
                .artists
                .iter()
                .any(|entry| &entry.name == artist)
        {
            self.music_route = MusicRoute::Artists;
        }

        if let MusicRoute::Album { key } = &self.music_route
            && !self.library.albums.iter().any(|album| &album.key == key)
        {
            self.music_route = MusicRoute::Artists;
        }

        if self
            .overview_album
            .as_ref()
            .is_some_and(|key| !self.library.albums.iter().any(|album| &album.key == key))
        {
            self.overview_album = None;
        }

        match self.video_route.clone() {
            VideoRoute::Seasons { series } if self.video_library.series(&series).is_none() => {
                self.video_route = VideoRoute::Series;
            }
            VideoRoute::Episodes { series, season } => {
                let Some(entry) = self.video_library.series(&series) else {
                    self.video_route = VideoRoute::Series;
                    return;
                };

                if !entry.seasons.iter().any(|entry| entry.number == season) {
                    self.video_route = VideoRoute::Seasons { series };
                }
            }
            _ => {}
        }
    }

    fn aliases_content(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut aliases: Vec<(&String, &String)> = self.aliases.iter().collect();
        aliases.sort_by_key(|(alias, _)| alias.to_lowercase());

        let mut list = widget::list_column();

        if aliases.is_empty() {
            list = list.add(widget::text::body(fl!("aliases-empty")));
        } else {
            for (alias, canonical) in aliases {
                list = list.add(
                    widget::row::with_capacity(4)
                        .push(widget::text::body(alias))
                        .push(widget::text::caption("→"))
                        .push(widget::text::body(canonical))
                        .push(widget::space::horizontal())
                        .push(
                            widget::button::text(fl!("remove"))
                                .on_press(Message::DeleteAlias(alias.clone())),
                        )
                        .spacing(spacing.space_xs)
                        .align_y(cosmic::iced::Alignment::Center)
                        .width(Length::Fill),
                );
            }
        }

        let can_save =
            !self.alias_input.trim().is_empty() && !self.canonical_input.trim().is_empty();

        let mut content = widget::column::with_capacity(9)
            .spacing(spacing.space_s)
            .push(widget::text::body(fl!("aliases-help")))
            .push(list)
            .push(widget::text::caption(fl!("alias-original")))
            .push(
                widget::text_input::text_input(
                    fl!("alias-original-placeholder"),
                    &self.alias_input,
                )
                .on_input(Message::AliasChanged),
            )
            .push(widget::text::caption(fl!("alias-canonical")))
            .push(
                widget::text_input::text_input(
                    fl!("alias-canonical-placeholder"),
                    &self.canonical_input,
                )
                .on_input(Message::CanonicalChanged),
            )
            .push(
                widget::button::suggested(fl!("add-alias"))
                    .on_press_maybe(can_save.then_some(Message::SaveAlias)),
            );

        if let Some(error) = &self.alias_error {
            content = content.push(widget::warning(error.clone()));
        }

        content.into()
    }

    fn details_content(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let Some(details) = &self.details else {
            return widget::text::body(fl!("details-empty")).into();
        };

        let mut content = widget::column::with_capacity(5).spacing(spacing.space_s);
        let cover_target = covers::CoverTarget::for_kind(details.kind);

        if details.cover_ready
            && let Some(hash) = &details.cover_hash
            && !self.cache_dir.as_os_str().is_empty()
        {
            content = content.push(
                widget::image(cosmic::iced::widget::image::Handle::from_path(
                    covers::thumbnail_path(&self.cache_dir, hash, cover_target),
                ))
                .width(Length::Fixed(240.0))
                .height(Length::Fixed(cover_target.height_for_width(240.0)))
                .content_fit(cosmic::iced::ContentFit::Cover)
                .border_radius(cosmic::theme::active().cosmic().corner_radii.radius_s),
            );
        }

        content
            .push(widget::text::title3(&details.title))
            .push(widget::text::body(&details.subtitle))
            .push(
                widget::button::suggested(fl!("open")).on_press(Message::OpenItem(details.clone())),
            )
            .push(widget::text::caption(fl!("path")))
            .push(widget::selectable_text::body(&details.path))
            .into()
    }
}

/// Solo se abre una URL https con esquema y destino no vacíos.
fn is_https_url(url: &str) -> bool {
    let url = url.trim();

    url.strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty())
}

fn repository_url() -> Option<&'static str> {
    repository_url_from(REPOSITORY)
}

fn repository_url_from(value: &str) -> Option<&str> {
    let url = value.trim();

    is_https_url(url).then_some(url)
}

async fn open_file(path: PathBuf) -> Result<(), String> {
    match tokio::task::spawn_blocking(move || opener::open_path(&path)).await {
        Ok(result) => result,
        Err(error) => Err(format!("No se pudo abrir el archivo: {error}")),
    }
}

async fn choose_catalog_folder() -> Result<Option<PathBuf>, String> {
    use cosmic::dialog::file_chooser;

    let dialog = file_chooser::open::Dialog::new().title(fl!("catalog-folder"));

    match dialog.open_folder().await {
        Ok(response) => response
            .url()
            .to_file_path()
            .map(Some)
            .map_err(|()| fl!("catalog-folder-invalid")),
        Err(file_chooser::Error::Cancelled) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Rutas que el portal de documentos de Flatpak expone dentro del sandbox.
/// Son efímeras (cambian al re-conceder permisos) y no se pueden traducir a
/// la ruta del host con fiabilidad: con acceso directo al sistema de archivos
/// ya concedido, la raíz correcta es siempre una ruta del host.
const DOCUMENT_PORTAL_PREFIX: &str = "/run/flatpak/";

fn catalog_root_from_path(path: &Path) -> Result<String, String> {
    // 1-2. Canonicalizar la ruta del selector: resuelve symlinks y montajes
    // del portal, y garantiza que las comparaciones de prefijo sean exactas.
    let canonical =
        fs::canonicalize(path).map_err(|_| fl!("catalog-folder-portal"))?;

    // 3-4. Si después de canonicalizar sigue siendo una ruta del portal,
    // la carpeta no es utilizable: pedir una carpeta real.
    if canonical
        .to_str()
        .is_some_and(|text| text.starts_with(DOCUMENT_PORTAL_PREFIX))
    {
        return Err(fl!("catalog-folder-portal"));
    }

    let root = canonical
        .to_str()
        .ok_or_else(|| fl!("catalog-folder-not-utf8"))?;

    if root.trim().is_empty() {
        return Err(fl!("catalog-folder-invalid"));
    }

    // Las reglas existentes se aplican sobre la ruta canónica:
    // is_prohibited_root rechaza "/" y el $HOME EXACTO únicamente; las
    // subcarpetas como /home/<usuario>/Disco_Canal siguen siendo válidas.
    if is_prohibited_root(&canonical) {
        return Err(fl!("catalog-folder-too-broad"));
    }

    // 5. Ruta válida: devolverla para guardar.
    Ok(root.to_owned())
}

/// Rechaza la raíz del sistema y la carpeta personal completa: catalogarlas
/// recorrería casi todo el disco y `delete_missing` tendría un alcance enorme.
fn is_prohibited_root(path: &Path) -> bool {
    if path == Path::new("/") {
        return true;
    }

    let Some(home) = dirs::home_dir() else {
        return false;
    };

    if path == home {
        return true;
    }

    match (fs::canonicalize(path), fs::canonicalize(&home)) {
        (Ok(path), Ok(home)) => path == home,
        _ => false,
    }
}

async fn load_cached() -> Result<CachedCatalog, String> {
    match tokio::task::spawn_blocking(db::load_cached).await {
        Ok(result) => result,
        Err(error) => Err(format!("No se pudo cargar el catálogo guardado: {error}")),
    }
}

async fn save_alias(alias: String, canonical: String) -> Result<AliasMap, String> {
    match tokio::task::spawn_blocking(move || db::set_alias_and_load(&alias, &canonical)).await {
        Ok(result) => result,
        Err(error) => Err(format!("No se pudo guardar el alias: {error}")),
    }
}

async fn delete_alias(alias: String) -> Result<AliasMap, String> {
    match tokio::task::spawn_blocking(move || db::remove_alias_and_load(&alias)).await {
        Ok(result) => result,
        Err(error) => Err(format!("No se pudo borrar el alias: {error}")),
    }
}

/// The page to display in the application.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Page {
    #[default]
    All,
    Music,
    Video,
    Books,
}

impl Page {
    /// Localized label used by both the nav bar and the page title.
    pub fn label(self) -> String {
        match self {
            Self::All => fl!("nav-all"),
            Self::Music => fl!("nav-music"),
            Self::Video => fl!("nav-video"),
            Self::Books => fl!("nav-books"),
        }
    }
}

/// Ruta dentro de la biblioteca musical.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum MusicRoute {
    #[default]
    Artists,
    Albums {
        artist: String,
    },
    Album {
        key: AlbumKey,
    },
}

/// Ruta dentro de la biblioteca de video.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum VideoRoute {
    #[default]
    Series,
    Seasons {
        series: String,
    },
    Episodes {
        series: String,
        season: u32,
    },
}

/// State of the background catalog scan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ScanState {
    #[default]
    Idle,
    Running,
    Failed(String),
}

/// The context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
    Aliases,
    Details,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    Aliases,
    CatalogFolder,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::Aliases => Message::ToggleContextPage(ContextPage::Aliases),
            MenuAction::CatalogFolder => Message::ChooseCatalogFolder,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;

    fn app_con_detalles_abiertos() -> AppModel {
        let mut app = AppModel {
            core: cosmic::Core::default(),
            context_page: ContextPage::Details,
            about: About::default(),
            nav: nav_bar::Model::default(),
            key_binds: HashMap::new(),
            config: Config::default(),
            items: Vec::new(),
            library: Library::default(),
            aliases: AliasMap::new(),
            video_library: VideoLibrary::default(),
            video_route: VideoRoute::default(),
            scan_state: ScanState::Idle,
            deleted: 0,
            warning: None,
            search: String::new(),
            music_route: MusicRoute::Albums {
                artist: String::from("Doechii"),
            },
            overview_album: None,
            details: Some(Details {
                id: 1,
                title: String::from("Canción"),
                subtitle: String::from("Doechii · Álbum"),
                path: String::from("/musica/cancion.mp3"),
                cover_hash: None,
                cover_ready: false,
                kind: Kind::Music,
                open_action: OpenAction::XdgOpen,
            }),
            pending_click: None,
            click_seq: 0,
            alias_input: String::new(),
            canonical_input: String::new(),
            alias_error: None,
            cache_dir: PathBuf::new(),
        };
        app.core.window.show_context = true;
        app
    }

    #[test]
    fn regresar_un_nivel_navega_y_cierra_detalles() {
        let mut app = app_con_detalles_abiertos();

        let _ = app.update(Message::GoToArtists);

        assert_eq!(app.music_route, MusicRoute::Artists);
        assert!(app.details.is_none());
        assert!(!app.core.window.show_context);
    }

    #[test]
    fn raiz_vacia_no_escanea() {
        let mut app = app_con_detalles_abiertos();
        app.scan_state = ScanState::Idle;

        let _ = app.update(Message::ScanRequested);

        assert_eq!(app.scan_state, ScanState::Idle);
        assert_eq!(app.items.len(), 0);
    }

    #[test]
    fn aplicar_raiz_pide_escaneo_y_no_lo_duplica() {
        let mut app = app_con_detalles_abiertos();
        let config = Config {
            catalog_root: String::from("/tmp/catalogo"),
            view_mode: String::from("grid"),
        };

        let _ = app.apply_catalog_root(config.clone());

        assert_eq!(app.config.catalog_root, "/tmp/catalogo");
        assert_eq!(app.scan_state, ScanState::Running);
        assert!(app.details.is_none());

        app.scan_state = ScanState::Idle;
        let _ = app.update(Message::UpdateConfig(config));

        assert_eq!(app.scan_state, ScanState::Idle);
    }

    #[test]
    fn cancelar_el_selector_no_cambia_nada() {
        let mut app = app_con_detalles_abiertos();

        let _ = app.update(Message::CatalogFolderChosen(Ok(None)));

        assert_eq!(app.config.catalog_root, "");
        assert!(app.warning.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn rechaza_rutas_no_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = Path::new(OsStr::from_bytes(b"/tmp/catalogo-\xff"));
        let error = catalog_root_from_path(path).unwrap_err();

        assert_ne!(error, "");
    }

    #[test]
    fn rechaza_la_raiz_del_sistema() {
        let error = catalog_root_from_path(Path::new("/")).unwrap_err();

        assert_eq!(error, fl!("catalog-folder-too-broad"));
    }

    #[test]
    fn rechaza_el_directorio_home() {
        if let Some(home) = dirs::home_dir() {
            let error = catalog_root_from_path(&home).unwrap_err();

            assert_eq!(error, fl!("catalog-folder-too-broad"));
        }
    }

    #[test]
    fn acepta_una_carpeta_normal() {
        // La raíz se canonicaliza antes de guardarse: debe existir en disco.
        let base = std::env::temp_dir().join(format!("mi-catalogo-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let canonical = fs::canonicalize(&base).unwrap().to_string_lossy().into_owned();
        assert_eq!(
            catalog_root_from_path(&base).unwrap(),
            canonical,
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn solo_abre_urls_https_con_destino() {
        assert!(is_https_url("https://github.com/ejemplo/repo"));
        assert!(!is_https_url("http://github.com/ejemplo/repo"));
        assert!(!is_https_url("file:///etc/passwd"));
        assert!(!is_https_url("javascript:alert(1)"));
        assert!(!is_https_url("https://"));
        assert!(!is_https_url(""));
    }

    #[test]
    fn omite_el_enlace_si_el_repositorio_no_es_https() {
        assert_eq!(repository_url_from("TODO: URL del repositorio"), None);
        assert_eq!(repository_url_from("http://github.com/ejemplo/repo"), None);
        assert_eq!(
            repository_url_from("https://github.com/ejemplo/repo"),
            Some("https://github.com/ejemplo/repo")
        );
    }

    #[test]
    fn valida_raices_portal_prohibidas_y_subcarpetas() {
        // 1. Rutas del portal: rechazadas. (Si no existen, falla canonicalize;
        //    si existen, falla el prefijo. El resultado debe ser Err en ambos casos.)
        assert!(
            catalog_root_from_path(Path::new("/run/flatpak/doc/00000000/Disco_Canal")).is_err()
        );

        // 2. La raíz del sistema y el $HOME exacto siguen prohibidos (regla existente).
        assert!(catalog_root_from_path(Path::new("/")).is_err());
        if let Some(home) = dirs::home_dir() {
            assert!(catalog_root_from_path(&home).is_err());
        }

        // 3. Una subcarpeta real de $HOME es válida (regla que NO debe romperse).
        let base = std::env::temp_dir().join(format!("mi-catalogo-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        assert!(catalog_root_from_path(&base).is_ok());
        let _ = fs::remove_dir_all(&base);

        // 4. Una carpeta inexistente falla: canonicalize no puede resolverla.
        assert!(
            catalog_root_from_path(Path::new("/definitivamente/no/existe/12345")).is_err()
        );
    }
}
