use iced::highlighter;
use iced::widget::text::Wrapping;
use iced::mouse::ScrollDelta;
use iced::widget::{button, column, container, image, mouse_area, pick_list, row, scrollable, text, text_editor, text_input, Row, Space};
use iced::{Element, Font, Length, Point};
use onevs100_core::audio::Player;
use onevs100_core::assets::{self, Imported, Store};
use onevs100_core::browser::Browser;
use onevs100_core::inspect::{is_supported_name, Preview};
use onevs100_core::figure::{self, Expression};
use onevs100_core::mct;
use onevs100_core::render::{Scene, TintPreset};
use onevs100_core::viewer::ViewState;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CONTROLS_MAX_HEIGHT: f32 = 150.0;
const AUDIO_LIST_HEIGHT: f32 = 170.0;
const DRAG_RESOLUTION: u32 = 224;
const FULL_RESOLUTION: u32 = 384;
const LINES_PER_NOTCH: f32 = 1.0;
const PIXELS_PER_NOTCH: f32 = 40.0;
const HELP: [&str; 9] = [
    "Click a folder, package (LIVE) or archive (.xzp) to go inside it.",
    "Click any file to see its details; images and textures show a preview.",
    "Use Back to leave a package or archive, Up to leave a folder.",
    "Text files (lua, resx, xml, txt) open in a read-only highlighted viewer.",
    "Videos show a frame; use the frame-at buttons to jump around (needs ffmpeg).",
    "Wave banks and sound banks list their sounds; press play (needs ffplay with XMA support).",
    "Packages and executables list their metadata (title id, versions, libraries).",
    "Export saves the selected file, or the shown texture as PNG, to the export folder.",
    "Type a folder path above and press Enter to jump anywhere.",
];

struct App {
    browser: Browser,
    path_input: String,
    export_dir: String,
    status: String,
    body: text_editor::Content,
    player: Player,
    view: ViewState,
    live: Option<Preview>,
    pointer: [f32; 2],
    poses: Vec<PoseChoice>,
    pose: PoseChoice,
    expression: Expression,
    tint: TintPreset,
    styled: Option<Arc<Scene>>,
    setup: Option<Setup>,
}

#[derive(Default)]
struct Setup {
    path_input: String,
    error: Option<String>,
    busy: bool,
}

const STAND_POSE: &str = "Animation Generic Stand 0";

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoseChoice {
    id: Option<usize>,
    name: String,
}

impl std::fmt::Display for PoseChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

fn pose_choices() -> Vec<PoseChoice> {
    let rest = PoseChoice { id: None, name: "T-pose".into() };
    let mut poses: Vec<PoseChoice> = onevs100_core::body::pack_bytes().map(figure::poses).unwrap_or_default().into_iter().map(|(id, name)| PoseChoice { id: Some(id), name }).collect();
    poses.insert(0, rest);
    poses
}

fn default_pose(poses: &[PoseChoice]) -> PoseChoice {
    poses.iter().find(|p| p.name == STAND_POSE).or_else(|| poses.first()).cloned().unwrap_or(PoseChoice { id: None, name: "T-pose".into() })
}

#[derive(Debug, Clone)]
enum Message {
    PathChanged(String),
    Go,
    Back,
    Up,
    Open(usize),
    Texture(usize),
    ExportDirChanged(String),
    ExportFile,
    ExportPng,
    Seek(f64),
    Editor(text_editor::Action),
    Decoded(String, Result<Preview, String>),
    Play(usize),
    Stop,
    ViewPress,
    ViewMove(Point),
    ViewRelease,
    ViewScroll(ScrollDelta),
    ViewReset,
    PoseChosen(PoseChoice),
    ExpressionChosen(Expression),
    TintChosen(TintPreset),
    SetupPathChanged(String),
    SetupBrowse(bool),
    SetupPicked(Option<PathBuf>),
    SetupImport,
    SetupDone(Result<Imported, String>),
}

fn default_export_dir() -> String {
    std::env::var("HOME").map_or_else(|_| "onevs100_export".into(), |h| format!("{h}/onevs100_export"))
}

impl App {
    fn new() -> Self {
        let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let browser = Browser::new(&start).expect("current directory is readable");
        let poses = pose_choices();
        Self { browser, path_input: start.display().to_string(), export_dir: default_export_dir(), status: "Pick a folder or file on the left.".into(), body: text_editor::Content::new(), player: Player::new(), view: ViewState::default(), live: None, pointer: [0.0; 2], pose: default_pose(&poses), poses, expression: Expression::Neutral, tint: TintPreset::Original, styled: None, setup: (!assets::available()).then(Setup::default) }
    }

    fn styled_figure(&mut self) {
        let Some(detail) = self.browser.detail() else { return };
        if let (Some(figure), Some(pack)) = (&detail.figure, onevs100_core::body::pack_bytes()) {
            match figure.0.styled(pack, self.pose.id, self.expression) {
                Ok(scene) => self.styled = Some(Arc::new(scene)),
                Err(e) => self.status = format!("Could not apply the look: {e}"),
            }
        } else if let Some(scene) = &detail.scene {
            self.styled = self.tint.colours().map(|c| Arc::new(scene.0.tinted(c)));
        }
        self.redraw(FULL_RESOLUTION);
    }

    fn redraw(&mut self, size: u32) {
        let started = std::time::Instant::now();
        let camera = self.view.camera;
        self.live = Some(match (&self.styled, self.browser.detail().and_then(|d| d.scene.as_ref())) {
            (Some(styled), _) => styled.render(&camera, size, size),
            (None, Some(original)) => original.0.render(&camera, size, size),
            (None, None) => return,
        });
        self.status = format!("View rendered in {} ms.", started.elapsed().as_millis());
    }

    fn _redraw_original(&mut self, size: u32) {
        let Some(scene) = self.browser.detail().and_then(|d| d.scene.clone()) else { return };
        let started = std::time::Instant::now();
        self.live = Some(scene.0.render(&self.view.camera, size, size));
        self.status = format!("View rendered in {} ms.", started.elapsed().as_millis());
    }

    fn start_import(&mut self) -> iced::Task<Message> {
        let Some(setup) = &mut self.setup else { return iced::Task::none() };
        let path = PathBuf::from(setup.path_input.trim().trim_matches('\''));
        setup.busy = true;
        setup.error = None;
        iced::Task::perform(async move { Store::user().import(&path).map_err(|e| format!("{e:#}")) }, Message::SetupDone)
    }

    fn finish_import(&mut self, result: Result<Imported, String>) {
        match result {
            Ok(done) => {
                self.setup = None;
                self.poses = pose_choices();
                self.pose = default_pose(&self.poses);
                self.status = format!("Imported the avatar assets ({} bytes) from {}.", done.bytes, done.source);
            }
            Err(e) => {
                if let Some(setup) = &mut self.setup {
                    setup.busy = false;
                    setup.error = Some(e);
                }
            }
        }
    }

    fn setup_view(&self, setup: &Setup) -> Element<'_, Message> {
        let ready = !setup.busy && !setup.path_input.trim().is_empty();
        let mut import = button("Import");
        if ready {
            import = import.on_press(Message::SetupImport);
        }
        let mut column = column![
            text("Avatar assets required").size(24),
            text(assets::REQUIREMENT).size(14).width(Length::Fill).wrapping(Wrapping::Word),
            row![button("Choose file…").on_press(Message::SetupBrowse(false)), button("Choose folder…").on_press(Message::SetupBrowse(true))].spacing(8),
            row![text_input("path to the update zip, folder, package or AvatarAssetPack.toc", &setup.path_input).on_input(Message::SetupPathChanged).on_submit(Message::SetupImport), import].spacing(8),
            text(format!("Stored in {}", Store::user().dir().display())).size(13),
        ]
        .spacing(12);
        if setup.busy {
            column = column.push(text("Importing…").size(14));
        }
        if let Some(error) = &setup.error {
            column = column.push(text(error.clone()).size(14).width(Length::Fill).wrapping(Wrapping::WordOrGlyph));
        }
        container(scrollable(column.width(Length::Fill))).padding(24).width(Length::Fill).height(Length::Fill).into()
    }

    fn report<T>(&mut self, result: anyhow::Result<T>, ok: impl FnOnce(T) -> String) {
        self.status = match result {
            Ok(v) => ok(v),
            Err(e) => format!("Error: {e}"),
        };
    }

    fn load_body(&mut self) {
        let content = self.browser.detail().and_then(|d| d.text.as_ref()).map(|t| t.content.as_str()).unwrap_or_default();
        self.body = text_editor::Content::with_text(content);
    }

    fn decode_task(&mut self) -> iced::Task<Message> {
        let Some((name, data)) = self.browser.pending_preview() else { return iced::Task::none() };
        self.status = format!("Decoding {name} with the game's own decoder (can take ~20 s for large textures)...");
        iced::Task::perform(async move { (name, mct::decode(&data).map_err(|e| e.to_string())) }, |(name, result)| Message::Decoded(name, result))
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::SetupPathChanged(path) => {
                if let Some(setup) = &mut self.setup {
                    setup.path_input = path;
                }
                return iced::Task::none();
            }
            Message::SetupBrowse(folder) => {
                return iced::Task::perform(
                    async move {
                        let dialog = rfd::AsyncFileDialog::new().set_title("Select the 17559 system update");
                        if folder { dialog.pick_folder().await } else { dialog.pick_file().await }.map(|h| h.path().to_path_buf())
                    },
                    Message::SetupPicked,
                );
            }
            Message::SetupPicked(Some(path)) => {
                if let Some(setup) = &mut self.setup {
                    setup.path_input = path.display().to_string();
                }
                return self.start_import();
            }
            Message::SetupPicked(None) => return iced::Task::none(),
            Message::SetupImport => return self.start_import(),
            Message::SetupDone(result) => {
                self.finish_import(result);
                return iced::Task::none();
            }
            _ => {}
        }
        let opens = matches!(message, Message::Open(_));
        self.apply(message);
        if opens { self.decode_task() } else { iced::Task::none() }
    }

    fn apply(&mut self, message: Message) {
        match message {
            Message::PathChanged(s) => self.path_input = s,
            Message::Go => {
                let r = self.browser.go_to(&PathBuf::from(&self.path_input));
                self.report(r, |_| "Opened folder.".into());
                self.load_body();
            }
            Message::Back => {
                self.status = if self.browser.back() { "Went back.".into() } else { "Already at the top of this folder.".into() };
                self.load_body();
            }
            Message::Up => {
                self.status = if self.browser.up() { "Went up.".into() } else { "Cannot go higher.".into() };
                self.load_body();
            }
            Message::ViewPress => self.view.press(self.pointer),
            Message::ViewMove(p) => {
                self.pointer = [p.x, p.y];
                if self.view.drag([p.x, p.y]) {
                    self.redraw(DRAG_RESOLUTION);
                }
            }
            Message::ViewRelease => {
                if self.view.dragging() {
                    self.view.release();
                    self.redraw(FULL_RESOLUTION);
                }
            }
            Message::ViewScroll(delta) => {
                let notches = match delta {
                    ScrollDelta::Lines { y, .. } => y / LINES_PER_NOTCH,
                    ScrollDelta::Pixels { y, .. } => y / PIXELS_PER_NOTCH,
                };
                if self.view.scroll(notches) {
                    self.redraw(FULL_RESOLUTION);
                }
            }
            Message::ViewReset => {
                self.view.reset();
                self.redraw(FULL_RESOLUTION);
            }
            Message::PoseChosen(p) => {
                self.pose = p;
                self.styled_figure();
            }
            Message::ExpressionChosen(e) => {
                self.expression = e;
                self.styled_figure();
            }
            Message::TintChosen(t) => {
                self.tint = t;
                self.styled_figure();
            }
            Message::Open(i) => {
                self.view.reset();
                self.live = None;
                self.styled = None;
                self.pose = default_pose(&self.poses);
                self.expression = Expression::Neutral;
                self.tint = TintPreset::Original;
                let r = self.browser.open(i);
                self.report(r, |_| String::new());
                self.load_body();
            }
            Message::Seek(fraction) => {
                let r = self.browser.seek_video(fraction);
                self.report(r, |_| format!("Showing frame at {:.0}%.", fraction * 100.0));
            }
            Message::Decoded(name, result) => match result {
                Ok(preview) => {
                    let shown = self.browser.set_preview(&name, preview);
                    self.status = if shown { format!("Decoded {name}.") } else { String::new() };
                }
                Err(e) => self.status = format!("Could not decode {name}: {e}"),
            },
            Message::Play(i) => {
                let r = self.browser.audio_wav(i).and_then(|wav| self.player.play(&wav));
                self.report(r, |_| format!("Playing item {i}."));
            }
            Message::SetupPathChanged(_) | Message::SetupBrowse(_) | Message::SetupPicked(_) | Message::SetupImport | Message::SetupDone(_) => {}
            Message::Stop => {
                self.player.stop();
                self.status = "Stopped.".into();
            }
            Message::Editor(action) => {
                if !action.is_edit() {
                    self.body.perform(action);
                }
            }
            Message::Texture(i) => {
                let r = self.browser.select_texture(i);
                self.report(r, |_| format!("Showing texture {i}."));
            }
            Message::ExportDirChanged(s) => self.export_dir = s,
            Message::ExportFile => {
                let r = self.browser.export_selected(Path::new(&self.export_dir));
                self.report(r, |p| format!("Saved {}", p.display()));
            }
            Message::ExportPng => {
                let r = self.browser.export_texture_png(Path::new(&self.export_dir));
                self.report(r, |p| format!("Saved {}", p.display()));
            }
        }
    }

    fn toolbar(&self) -> Element<'_, Message> {
        column![
            row![button("Back").on_press(Message::Back), button("Up").on_press(Message::Up), text_input("folder path", &self.path_input).on_input(Message::PathChanged).on_submit(Message::Go), button("Go").on_press(Message::Go)].spacing(6),
            text(self.browser.breadcrumb()).size(13).width(Length::Fill).wrapping(Wrapping::WordOrGlyph),
        ]
        .spacing(6)
        .into()
    }

    fn listing(&self) -> Element<'_, Message> {
        let rows = self.browser.entries().iter().enumerate().fold(column![].spacing(2), |col, (i, e)| {
            let label = if e.is_dir { format!("▸ {}", e.name) } else { e.name.clone() };
            let mut name = text(label).size(14).width(Length::Fill).wrapping(Wrapping::WordOrGlyph);
            if !e.is_dir && !is_supported_name(&e.name) {
                name = name.style(text::secondary);
            }
            col.push(button(name).width(Length::Fill).on_press(Message::Open(i)))
        });
        scrollable(rows).width(Length::FillPortion(2)).height(Length::Fill).into()
    }

    fn controls(&self) -> Element<'_, Message> {
        let mut export = vec![button("Export file").on_press(Message::ExportFile).into()];
        if self.browser.texture_count() > 0 {
            export.push(button("Export texture as PNG").on_press(Message::ExportPng).into());
        }
        let textures: Vec<Element<'_, Message>> = (0..self.browser.texture_count()).map(|i| button(text(format!("texture {i}")).size(13)).on_press(Message::Texture(i)).into()).collect();
        let seek: Vec<Element<'_, Message>> = if self.browser.video_seconds().is_some() {
            [0.0, 0.25, 0.5, 0.75, 0.95].iter().map(|f| button(text(format!("frame {:.0}%", f * 100.0)).size(13)).on_press(Message::Seek(*f)).into()).collect()
        } else {
            Vec::new()
        };
        let mut pieces: Vec<Element<'_, Message>> = vec![
            wrapped(export),
            row![text("Export folder:").size(13), text_input("export folder", &self.export_dir).on_input(Message::ExportDirChanged)].spacing(6).into(),
        ];
        for group in [textures, seek] {
            if !group.is_empty() {
                pieces.push(wrapped(group));
            }
        }
        container(scrollable(column(pieces).spacing(6).width(Length::Fill)).width(Length::Fill)).max_height(CONTROLS_MAX_HEIGHT).into()
    }

    fn audio_panel(&self) -> Element<'_, Message> {
        let labels = self.browser.audio_labels();
        if labels.is_empty() {
            return Space::new().into();
        }
        let rows = labels.into_iter().enumerate().fold(column![].spacing(2), |c, (i, l)| c.push(button(text(format!("▶ {l}")).size(13).width(Length::Fill).wrapping(Wrapping::WordOrGlyph)).width(Length::Fill).on_press(Message::Play(i))));
        column![button("Stop").on_press(Message::Stop), scrollable(rows).width(Length::Fill).height(Length::Fixed(AUDIO_LIST_HEIGHT))].spacing(4).into()
    }

    fn details(&self) -> Element<'_, Message> {
        let Some(detail) = self.browser.detail() else {
            let help = HELP.iter().fold(column![text("How to use").size(20)].spacing(6), |c, l| c.push(text(*l).size(14).width(Length::Fill).wrapping(Wrapping::Word)));
            return container(scrollable(help)).width(Length::FillPortion(3)).height(Length::Fill).padding(8).into();
        };
        let shown = if detail.scene.is_some() { self.live.as_ref().or(detail.image.as_ref()) } else { detail.image.as_ref() };
        let bitmap: Element<'_, Message> = shown.map_or_else(
            || Space::new().into(),
            |p| {
                let handle = image::Handle::from_rgba(p.width, p.height, p.rgba.clone());
                if detail.scene.is_some() {
                    image(handle).width(Length::Fixed(FULL_RESOLUTION as f32)).height(Length::Fixed(FULL_RESOLUTION as f32)).into()
                } else {
                    image(handle).width(Length::Fill).content_fit(iced::ContentFit::ScaleDown).into()
                }
            },
        );
        let preview: Element<'_, Message> = if detail.scene.is_some() {
            let surface = mouse_area(bitmap).on_press(Message::ViewPress).on_move(Message::ViewMove).on_release(Message::ViewRelease).on_scroll(Message::ViewScroll);
            let mut options: Vec<Element<'_, Message>> = Vec::new();
            if detail.figure.is_some() {
                options.push(text("Pose").size(13).into());
                options.push(pick_list(&self.poses[..], Some(&self.pose), Message::PoseChosen).text_size(13).into());
                options.push(text("Expression").size(13).into());
                options.push(pick_list(&Expression::ALL[..], Some(&self.expression), Message::ExpressionChosen).text_size(13).into());
            } else if detail.scene.as_ref().is_some_and(|s| s.0.is_tintable()) {
                options.push(text("Tint").size(13).into());
                options.push(pick_list(&TintPreset::ALL[..], Some(&self.tint), Message::TintChosen).text_size(13).into());
            }
            column![
                container(surface).width(Length::Fill).center_x(Length::Fill),
                wrapped(options),
                row![button("Reset view").on_press(Message::ViewReset), text("Drag to rotate, scroll to zoom.").size(13)].spacing(8)
            ]
            .spacing(6)
            .into()
        } else {
            bitmap
        };
        let lines = detail.lines.iter().fold(column![].spacing(1).width(Length::Fill), |c, l| c.push(text(l.clone()).size(13).font(Font::MONOSPACE).width(Length::Fill).wrapping(Wrapping::WordOrGlyph)));
        let body: Element<'_, Message> = match &detail.text {
            Some(t) => column![
                lines,
                text_editor(&self.body).on_action(Message::Editor).font(Font::MONOSPACE).size(13).highlight(t.syntax, highlighter::Theme::Base16Ocean).height(Length::Fill)
            ]
            .spacing(8)
            .height(Length::Fill)
            .into(),
            None => scrollable(column![preview, lines].spacing(10).width(Length::Fill)).width(Length::Fill).height(Length::Fill).into(),
        };
        column![
            text(self.browser.selected_name().unwrap_or_default().to_owned()).size(18).width(Length::Fill).wrapping(Wrapping::WordOrGlyph),
            text(detail.kind).size(14),
            self.controls(),
            self.audio_panel(),
            body,
        ]
        .spacing(8)
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .into()
    }

    fn view(&self) -> Element<'_, Message> {
        if let Some(setup) = &self.setup {
            return self.setup_view(setup);
        }
        column![self.toolbar(), row![self.listing(), self.details()].spacing(12).height(Length::Fill), text(self.status.clone()).size(13)].spacing(10).padding(12).into()
    }
}

fn wrapped<'a>(items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    Row::with_children(items).spacing(6).wrap().vertical_spacing(6).into()
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view).title("1 vs 100 Content Package Editor").window_size((1180.0, 760.0)).run()
}
