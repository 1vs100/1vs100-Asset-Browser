pub mod anim;
mod bitpack;
pub mod assets;
pub mod audio;
pub mod avatar;
pub mod browser;
pub mod caff;
pub mod inspect;
pub mod lzx;
pub mod model;
pub mod mct;
pub mod figure;
pub mod hiding;
pub mod pose;
pub mod render;
pub mod skeleton;
pub mod support;
pub mod shade;
pub mod source;
pub mod stfs;
pub mod strb;
pub mod texture;
pub mod timefmt;
pub mod video;
pub mod xdbf;
pub mod xex;
pub mod xcontent;
pub mod xenos;
pub mod xsb;
pub mod xwb;
pub mod xui;
pub mod xur;
pub mod blend;
pub mod body;
pub mod toc;
pub mod viewer;
pub mod ytgr;
mod xur_schema;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_packages;
#[cfg(test)]
mod tests_fixtures;
#[cfg(test)]
mod tests_browser;
#[cfg(test)]
mod tests_meta;
#[cfg(test)]
mod tests_avatar;
#[cfg(test)]
mod tests_xur;
#[cfg(test)]
mod tests_anim;
#[cfg(test)]
mod tests_shade;
#[cfg(test)]
mod tests_xex_vfs;
#[cfg(test)]
mod tests_assets;
#[cfg(test)]
mod tests_cover;
#[cfg(test)]
mod tests_viewer;
#[cfg(test)]
mod tests_camera;
#[cfg(test)]
mod tests_support;
#[cfg(test)]
mod tests_skeleton;
#[cfg(test)]
mod tests_toc;
#[cfg(test)]
mod tests_ytgr;
#[cfg(test)]
mod tests_audio;
#[cfg(test)]
mod tests_lzx;
#[cfg(test)]
mod tests_model;
