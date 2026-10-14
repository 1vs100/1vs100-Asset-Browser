pub mod avatar;
pub mod browser;
pub mod formats;

pub use avatar::{
    anim, blend, body, figure, hiding, model, pose, render, shade, skeleton, texture, xenos,
};
pub use browser::{assets, audio, inspect, mct, source, support, video, viewer};
pub use formats::{
    caff, lzx, stfs, strb, timefmt, toc, xcontent, xdbf, xex, xsb, xui, xur, xwb, ytgr,
};

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "tests/tests_anim.rs"]
mod tests_anim;
#[cfg(test)]
#[path = "tests/tests_assets.rs"]
mod tests_assets;
#[cfg(test)]
#[path = "tests/tests_audio.rs"]
mod tests_audio;
#[cfg(test)]
#[path = "tests/tests_avatar.rs"]
mod tests_avatar;
#[cfg(test)]
#[path = "tests/tests_browser.rs"]
mod tests_browser;
#[cfg(test)]
#[path = "tests/tests_camera.rs"]
mod tests_camera;
#[cfg(test)]
#[path = "tests/tests_cover.rs"]
mod tests_cover;
#[cfg(test)]
#[path = "tests/tests_fixtures.rs"]
mod tests_fixtures;
#[cfg(test)]
#[path = "tests/tests_lzx.rs"]
mod tests_lzx;
#[cfg(test)]
#[path = "tests/tests_meta.rs"]
mod tests_meta;
#[cfg(test)]
#[path = "tests/tests_model.rs"]
mod tests_model;
#[cfg(test)]
#[path = "tests/tests_packages.rs"]
mod tests_packages;
#[cfg(test)]
#[path = "tests/tests_shade.rs"]
mod tests_shade;
#[cfg(test)]
#[path = "tests/tests_skeleton.rs"]
mod tests_skeleton;
#[cfg(test)]
#[path = "tests/tests_support.rs"]
mod tests_support;
#[cfg(test)]
#[path = "tests/tests_toc.rs"]
mod tests_toc;
#[cfg(test)]
#[path = "tests/tests_viewer.rs"]
mod tests_viewer;
#[cfg(test)]
#[path = "tests/tests_xex_vfs.rs"]
mod tests_xex_vfs;
#[cfg(test)]
#[path = "tests/tests_xur.rs"]
mod tests_xur;
#[cfg(test)]
#[path = "tests/tests_ytgr.rs"]
mod tests_ytgr;
