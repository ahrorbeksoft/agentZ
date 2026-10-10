//! What t3code's sidebar shows about a project besides its name: an icon (one picked for it,
//! its favicon, or a colored monogram when it has neither). Each machine's server finds its
//! projects' icon files (`projects::ProjectStore::favicon`), as it reads the branches checked
//! out (`projects::ProjectStore::git_head`).

use std::path::PathBuf;
use std::sync::Arc;

use agentz_protocol::{Request, Response};
use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use gpui::{
    AnyElement, App, Asset, FontWeight, Hsla, ImageFormat, ImageSource, RenderImage, img, rgb,
};
use projects::{Project, ProjectIcon, ProjectId, WorkspaceKind};
use ui::{StyledImage as _, prelude::*};

use crate::attachment_image::{LoadFailure, is_online};
use crate::machines::{MachineId, Machines};

/// A worktree's or pasture's icon, wherever workspaces are listed.
pub fn workspace_icon(kind: WorkspaceKind) -> IconName {
    match kind {
        WorkspaceKind::Worktree => IconName::GitWorktree,
        WorkspaceKind::Pasture => IconName::Copy,
    }
}

/// The favicon its machine's server found for the project. This Mac's server found it on this
/// Mac's disk; another machine's sends it ([`Request::ProjectFavicon`]).
fn favicon(machine: MachineId, project: ProjectId, cx: &App) -> Option<ImageSource> {
    let path = Machines::global(cx)
        .read(cx)
        .projects(machine, cx)?
        .read(cx)
        .favicon(project)?
        .to_path_buf();
    if machine == MachineId::Local {
        return Some(path.into());
    }
    let request = RemoteFavicon {
        machine,
        project,
        path,
    };
    Some(ImageSource::Custom(Arc::new(move |window, cx| {
        let result = window.use_asset::<RemoteFaviconLoader>(&request, cx)?;
        match result {
            Ok(image) => Some(Ok(image)),
            Err(failure) => {
                // Asked for while the machine was offline: asked for again once it's back.
                if failure.while_offline && is_online(request.machine, cx) {
                    cx.remove_asset::<RemoteFaviconLoader>(&request);
                }
                Some(Err(failure.error))
            }
        }
    })))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RemoteFavicon {
    machine: MachineId,
    project: ProjectId,
    /// Where the server found it, so a different icon is fetched anew.
    path: PathBuf,
}

/// Fetches a project's favicon from its machine's server and decodes it.
enum RemoteFaviconLoader {}

impl Asset for RemoteFaviconLoader {
    type Source = RemoteFavicon;
    type Output = Result<Arc<RenderImage>, LoadFailure>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = Machines::global(cx).read(cx).client(source.machine, cx);
        let while_offline = !client
            .as_ref()
            .is_some_and(|client| client.read(cx).is_online());
        let response = client.map(|client| {
            client
                .read(cx)
                .request(Request::ProjectFavicon(source.project))
        });
        let svg_renderer = cx.svg_renderer();
        async move {
            let image = async {
                let response = response.context("the machine was removed")?.await?;
                let Response::ProjectFavicon(data) = response else {
                    return Err(anyhow!("unexpected response: {response:?}"));
                };
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .context("decoding the icon's base64")?;
                let format = image_format(&bytes)?;
                gpui::Image::from_bytes(format, bytes).to_image_data(svg_renderer)
            }
            .await;
            image.map_err(|error| {
                log::error!(
                    "failed to load the icon {}: {error:#}",
                    source.path.display()
                );
                LoadFailure {
                    error: error.into(),
                    while_offline,
                }
            })
        }
    }
}

/// An image's format, told from its bytes as GPUI's loader does for a file on disk, which takes
/// what isn't a known raster format for an SVG.
fn image_format(bytes: &[u8]) -> anyhow::Result<ImageFormat> {
    Ok(match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => ImageFormat::Png,
        Ok(image::ImageFormat::Jpeg) => ImageFormat::Jpeg,
        Ok(image::ImageFormat::WebP) => ImageFormat::Webp,
        Ok(image::ImageFormat::Gif) => ImageFormat::Gif,
        Ok(image::ImageFormat::Bmp) => ImageFormat::Bmp,
        Ok(image::ImageFormat::Tiff) => ImageFormat::Tiff,
        Ok(image::ImageFormat::Ico) => ImageFormat::Ico,
        Ok(image::ImageFormat::Pnm) => ImageFormat::Pnm,
        Ok(format) => return Err(anyhow!("unsupported image format: {format:?}")),
        Err(_) => ImageFormat::Svg,
    })
}

/// The icon, emoji or monogram picked in the project's settings, else its icon file (the one
/// picked, or the favicon found), else t3code's monogram tile (also shown while an image
/// loads, and when it fails to).
pub fn render_project_icon(
    machine: MachineId,
    project: &Project,
    size: Pixels,
    cx: &App,
) -> AnyElement {
    let name = project.name();
    let is_light = cx.theme().appearance().is_light();
    let font_family = theme::theme_settings(cx).buffer_font(cx).family.clone();
    match &project.icon {
        Some(ProjectIcon::Icon { name: icon, color }) => {
            if let Ok(icon) = icon.parse::<IconName>() {
                let color =
                    named_color(color, cx).unwrap_or_else(|| monogram_color(&name, is_light));
                return render_icon(icon, color, size).into_any_element();
            }
        }
        Some(ProjectIcon::Emoji { emoji }) => {
            return render_emoji(emoji.clone().into(), size).into_any_element();
        }
        Some(ProjectIcon::Monogram { text, color }) => {
            let color = named_color(color, cx).unwrap_or_else(|| monogram_color(&name, is_light));
            return render_monogram(monogram_text(text).into(), color, font_family, size)
                .into_any_element();
        }
        Some(ProjectIcon::Image { .. }) | None => {}
    }
    let text = SharedString::from(monogram(&name));
    let color = monogram_color(&name, is_light);
    let Some(image) = favicon(machine, project.id, cx) else {
        return render_monogram(text, color, font_family, size).into_any_element();
    };
    let placeholder =
        move || render_monogram(text.clone(), color, font_family.clone(), size).into_any_element();
    img(image)
        .size(size)
        .flex_none()
        .rounded_sm()
        .with_loading(placeholder.clone())
        .with_fallback(placeholder)
        .into_any_element()
}

/// One of the app's icons in its color, filling the space as t3code's Lucide icons do.
pub fn render_icon(icon: IconName, color: Hsla, size: Pixels) -> Div {
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(icon)
                .size(IconSize::Custom(rems_from_px(f32::from(size))))
                .color(Color::Custom(color)),
        )
}

/// An emoji at 80% of the space's height, as t3code sets one.
pub fn render_emoji(emoji: SharedString, size: Pixels) -> Div {
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .text_size(size * 0.8)
        .line_height(size)
        .child(emoji)
}

/// A monogram's letters as its tile shows them.
pub fn monogram_text(text: &str) -> String {
    text.chars().take(2).collect::<String>().to_uppercase()
}

/// t3code draws the monogram on a 16px tile with 8.25px text and a 25% corner radius.
pub fn render_monogram(
    text: SharedString,
    color: Hsla,
    font_family: SharedString,
    size: Pixels,
) -> Div {
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(size * 0.25)
        .bg(color.opacity(0.14))
        .text_color(color)
        .text_size(size * (8.25 / 16.))
        .line_height(size)
        .font_weight(FontWeight::BOLD)
        .font_family(font_family)
        .child(text)
}

/// Two letters for a project: the first letter, then a digit from the first word, the first
/// letter of the last word, or the first word's last letter (t3code's rule).
fn monogram(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let Some(first_glyph) = words.first().and_then(|word| word.chars().next()) else {
        return "PR".to_string();
    };
    let first_word: Vec<char> = words
        .first()
        .map(|word| word.chars().collect())
        .unwrap_or_default();
    let second_glyph = first_word
        .iter()
        .skip(1)
        .find(|glyph| glyph.is_numeric())
        .copied()
        .or_else(|| {
            if words.len() > 1 {
                words.last().and_then(|word| word.chars().next())
            } else {
                first_word.last().copied()
            }
        })
        .unwrap_or(first_glyph);
    format!("{first_glyph}{second_glyph}")
        .to_uppercase()
        .chars()
        .take(2)
        .collect()
}

/// t3code's project colors: Tailwind's 600 shades on light themes and 400 shades on dark ones.
pub const MONOGRAM_COLORS: [(&str, u32, u32); 18] = [
    ("gray", 0x4b5563, 0x9ca3af),
    ("red", 0xdc2626, 0xf87171),
    ("orange", 0xea580c, 0xfb923c),
    ("amber", 0xd97706, 0xfbbf24),
    ("yellow", 0xca8a04, 0xfacc15),
    ("lime", 0x65a30d, 0xa3e635),
    ("green", 0x16a34a, 0x4ade80),
    ("emerald", 0x059669, 0x34d399),
    ("teal", 0x0d9488, 0x2dd4bf),
    ("cyan", 0x0891b2, 0x22d3ee),
    ("sky", 0x0284c7, 0x38bdf8),
    ("blue", 0x2563eb, 0x60a5fa),
    ("indigo", 0x4f46e5, 0x818cf8),
    ("violet", 0x7c3aed, 0xa78bfa),
    ("purple", 0x9333ea, 0xc084fc),
    ("fuchsia", 0xc026d3, 0xe879f9),
    ("pink", 0xdb2777, 0xf472b6),
    ("rose", 0xe11d48, 0xfb7185),
];

/// The named color in the current theme's shade.
pub fn monogram_swatch(light: u32, dark: u32, cx: &App) -> Hsla {
    rgb(if cx.theme().appearance().is_light() {
        light
    } else {
        dark
    })
    .into()
}

/// One of [`MONOGRAM_COLORS`], by its name, in the current theme's shade.
pub fn named_color(name: &str, cx: &App) -> Option<Hsla> {
    MONOGRAM_COLORS
        .iter()
        .find(|(color, _, _)| *color == name)
        .map(|(_, light, dark)| monogram_swatch(*light, *dark, cx))
}

fn monogram_color(name: &str, is_light: bool) -> Hsla {
    let (_, light, dark) = MONOGRAM_COLORS
        .get(monogram_color_index(name))
        .copied()
        .unwrap_or(("blue", 0x2563eb, 0x60a5fa));
    rgb(if is_light { light } else { dark }).into()
}

/// The monogram and color name a project gets automatically, as a starting point for a
/// custom one.
pub fn automatic_monogram(name: &str) -> (String, &'static str) {
    let color = MONOGRAM_COLORS
        .get(monogram_color_index(name))
        .map_or("blue", |(color, _, _)| color);
    (monogram(name), color)
}

/// A stable color per project name, hashed the way t3code does.
fn monogram_color_index(name: &str) -> usize {
    let seed = name.trim().to_lowercase();
    let seed = if seed.is_empty() { "project" } else { &seed };
    seed.chars().fold(0, |index, glyph| {
        (index * 31 + glyph as usize) % MONOGRAM_COLORS.len()
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{Entity, TestAppContext};
    use projects::ProjectsSnapshot;

    use super::*;
    use crate::server_client::ServerClient;

    /// A 1 by 1 pixel PNG.
    const TINY_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    /// Project 1's icon on each machine, picked as given.
    struct ProjectIcons(Vec<(MachineId, Option<ProjectIcon>)>);

    impl Render for ProjectIcons {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            h_flex().children(self.0.iter().map(|(machine, icon)| {
                let project = Project {
                    id: ProjectId(1),
                    path: "/srv/demo".into(),
                    custom_name: None,
                    icon: icon.clone(),
                    workspaces: Vec::new(),
                    repository: None,
                };
                render_project_icon(*machine, &project, px(16.), cx)
            }))
        }
    }

    /// A client whose project 1 has a favicon, and the requests it was asked, which it
    /// answers with [`TINY_PNG`].
    fn client_with_favicon(
        machine: MachineId,
        cx: &mut App,
    ) -> (Entity<ServerClient>, Rc<RefCell<Vec<Request>>>) {
        let client = ServerClient::new_for_test(machine, "".into(), SpacesSnapshot::default(), cx);
        let requests = Rc::new(RefCell::new(Vec::new()));
        client.update(cx, |client, _| {
            let requests = requests.clone();
            client.answer_for_test(move |request| {
                requests.borrow_mut().push(request.clone());
                match request {
                    Request::ProjectFavicon(_) => Some(Response::ProjectFavicon(TINY_PNG.into())),
                    _ => None,
                }
            })
        });
        let projects = client.read(cx).projects().clone();
        projects.update(cx, |projects, cx| {
            projects.set_snapshot(
                ProjectsSnapshot {
                    favicons: vec![(ProjectId(1), "/srv/demo/favicon.png".into())],
                    ..Default::default()
                },
                cx,
            )
        });
        (client, requests)
    }

    /// Another machine's server sends the favicon it found; this Mac's is read from disk.
    #[gpui::test]
    fn remote_favicons_come_from_their_server(cx: &mut TestAppContext) {
        let remote = MachineId::Remote(1);
        let (local_requests, remote_requests) = cx.update(|cx| {
            crate::init_for_test(cx);
            let (local, local_requests) = client_with_favicon(MachineId::Local, cx);
            let (remote, remote_requests) = client_with_favicon(remote, cx);
            crate::machines::init_for_test(vec![local, remote], cx);
            (local_requests, remote_requests)
        });
        let (_view, cx) =
            cx.add_window_view(|_, _| ProjectIcons(vec![(MachineId::Local, None), (remote, None)]));
        cx.run_until_parked();
        assert!(local_requests.borrow().is_empty());
        assert_eq!(
            *remote_requests.borrow(),
            vec![Request::ProjectFavicon(ProjectId(1))]
        );
    }

    /// An icon, emoji or monogram picked is drawn as it is. A file picked is the icon file
    /// its server resolved, fetched as a favicon is, and so is an icon this build doesn't have.
    #[gpui::test]
    fn picked_icons_come_before_the_icon_file(cx: &mut TestAppContext) {
        let remote = MachineId::Remote(1);
        let requests = cx.update(|cx| {
            crate::init_for_test(cx);
            let (local, _) = client_with_favicon(MachineId::Local, cx);
            let (remote_client, requests) = client_with_favicon(remote, cx);
            crate::machines::init_for_test(vec![local, remote_client], cx);
            requests
        });
        let drawn = [
            ProjectIcon::Icon {
                name: "git_branch".into(),
                color: "teal".into(),
            },
            ProjectIcon::Emoji {
                emoji: "🚀".into()
            },
            ProjectIcon::Monogram {
                text: "q7".into(),
                color: "rose".into(),
            },
        ];
        let (view, cx) = cx.add_window_view(move |_, _| {
            ProjectIcons(drawn.map(|icon| (remote, Some(icon))).into())
        });
        cx.run_until_parked();
        assert!(requests.borrow().is_empty());

        let fetched = [
            ProjectIcon::Image {
                path: "assets/logo.png".into(),
            },
            ProjectIcon::Icon {
                name: "no_such_icon".into(),
                color: "teal".into(),
            },
        ];
        view.update(cx, |view, cx| {
            view.0 = fetched.map(|icon| (remote, Some(icon))).into();
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            *requests.borrow(),
            vec![Request::ProjectFavicon(ProjectId(1))]
        );
    }

    #[test]
    fn monograms() {
        assert_eq!(monogram("dramabot"), "DT");
        assert_eq!(monogram("shop-landing"), "SL");
        assert_eq!(monogram("web3app"), "W3");
        assert_eq!(monogram("x"), "XX");
        assert_eq!(monogram("--"), "PR");
    }

    #[test]
    fn monogram_colors_are_stable() {
        assert_eq!(
            monogram_color_index("agentZ"),
            monogram_color_index("AGENTZ")
        );
        assert!(monogram_color_index("anything") < MONOGRAM_COLORS.len());
    }

    #[test]
    fn favicon_formats_come_from_their_bytes() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        assert_eq!(image_format(png).ok(), Some(ImageFormat::Png));
        let ico = b"\0\0\x01\0\x01\0\x10\x10\0\0";
        assert_eq!(image_format(ico).ok(), Some(ImageFormat::Ico));
        assert_eq!(
            image_format(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#).ok(),
            Some(ImageFormat::Svg)
        );
    }
}
