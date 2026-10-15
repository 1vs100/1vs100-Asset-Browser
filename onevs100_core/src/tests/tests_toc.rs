use crate::{lzx, model, strb, toc};
use std::path::PathBuf;

fn pack() -> Vec<u8> {
    std::fs::read(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/avatar/AvatarAssetPack.toc"
    )))
    .unwrap()
}

#[test]
fn the_system_pack_splits_into_standalone_assets() {
    let d = pack();
    let assets = toc::split(&d);
    assert_eq!(assets.len(), 1102);
    let kinds = |t: u32| assets.iter().filter(|a| a.asset_type == t).count();
    assert_eq!(
        (kinds(1), kinds(2), kinds(3), kinds(4)),
        (588, 187, 267, 60)
    );
    assert_eq!(assets[0].offset, 0x9dfec);
}

#[test]
fn every_system_model_decodes() {
    let d = pack();
    let mut decoded = 0;
    for a in toc::split(&d).iter().filter(|a| a.asset_type == 1) {
        let s = strb::parse(a.data).unwrap();
        let b = s.blocks.iter().find(|b| b.id == 3).unwrap();
        let m =
            model::parse(&lzx::decompress(&a.data[b.start..b.start + b.length]).unwrap()).unwrap();
        decoded += usize::from(!m.batches.is_empty());
    }
    assert!(decoded > 500, "{decoded}");
}

#[test]
fn text_without_strb_magic_yields_nothing() {
    assert!(toc::split(&[0u8; 256]).is_empty());
}

#[test]
fn the_base_avatar_is_a_body_plus_a_head_per_gender() {
    let d = pack();
    let male = crate::body::from_pack(&d, crate::body::Gender::Male).unwrap();
    let female = crate::body::from_pack(&d, crate::body::Gender::Female).unwrap();
    assert_eq!((male.len(), female.len()), (2, 2));
    assert_ne!(
        male[0].batches[0].vertices.len(),
        female[0].batches[0].vertices.len()
    );
}

#[test]
fn an_item_preview_includes_the_base_body_when_the_pack_is_known() {
    let d = pack();
    let item = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/avatar/avataritems_raw/#Trending/Grumpy Cat Slippers/asset_v2.bin"
    ))
    .unwrap();
    let body = crate::body::from_pack(&d, crate::body::Gender::Male).unwrap();
    let y = crate::ytgr::parse(&item).unwrap();
    let m = model::parse(&lzx::decompress(y.block_data(&item, 3).unwrap()).unwrap()).unwrap();
    let alone = crate::render::preview(&m, 128, 128);
    let mut scene: Vec<&model::Model> = body.iter().collect();
    scene.push(&m);
    let composed = crate::render::preview_scene(&scene, 128, 128);
    let rows = |p: &crate::inspect::Preview| {
        let lit: Vec<usize> = p
            .rgba
            .chunks(4)
            .enumerate()
            .filter(|(_, c)| c[3] > 0)
            .map(|(i, _)| i / p.width as usize)
            .collect();
        lit.iter().max().unwrap() - lit.iter().min().unwrap()
    };
    assert!(
        rows(&alone) < rows(&composed) - 20 && rows(&composed) > 100,
        "{} {}",
        rows(&alone),
        rows(&composed)
    );
}

#[test]
fn toc_assets_open_as_entries_and_describe_themselves() {
    let d = pack();
    let kids = crate::source::expand_named("AvatarAssetPack.toc", &d)
        .unwrap()
        .unwrap();
    assert_eq!(kids.len(), 1102);
    assert_eq!(kids[0].name, "0000 MaleBody.type1.avatarasset");
    let det = crate::inspect::describe(&kids[0].name, &kids[0].payload.read().unwrap());
    assert_eq!(det.kind, "Avatar asset (STRB)");
    assert!(det.image.is_some());
    assert!(det.lines.iter().any(|l| l.starts_with("model: ")));
}

const FAKEREAL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../testdata/common/v2.0.1167.0.02-2009-12-07/media/avatars/fakereal.avd"
);

#[test]
fn an_avatar_metadata_blob_resolves_to_pack_models() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let models = crate::avatar::models_for(&avd, &d).unwrap();
    assert_eq!(models.len(), 5);
    assert!(models.iter().all(|m| !m.batches.is_empty()));
    assert!(crate::avatar::models_for(&avd[..10], &d).is_err());
}

#[test]
fn avatar_metadata_details_render_the_avatar_when_the_pack_is_known() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let det = crate::avatar::describe_avd_with(&avd, Some(&d)).unwrap();
    let image = det.image.unwrap();
    assert!(image.rgba.chunks(4).filter(|c| c[3] > 0).count() > 5000);
    assert!(det
        .lines
        .iter()
        .any(|l| l == "rendered 5 of 5 assets from the avatar pack"));
}

#[test]
fn texture_assets_decode_with_layers() {
    let d = pack();
    let a = toc::entry(&d, 761).unwrap();
    let s = strb::parse(a.data).unwrap();
    let b = s.blocks.iter().find(|b| b.id == 2).unwrap();
    let t = model::parse_texture(&lzx::decompress(&a.data[b.start..b.start + b.length]).unwrap())
        .unwrap();
    assert_eq!((t.width, t.height, t.layers), (128, 64, 14));
    let bytes = t.rgba_layer(0).unwrap();
    assert_eq!(bytes.len(), 128 * 64 * 4);
    assert!(t.rgba_layer(14).is_err());
    let b696 = toc::entry(&d, 696).unwrap();
    let s = strb::parse(b696.data).unwrap();
    let b = s.blocks.iter().find(|b| b.id == 2).unwrap();
    let t =
        model::parse_texture(&lzx::decompress(&b696.data[b.start..b.start + b.length]).unwrap())
            .unwrap();
    println!("PROBE 696 {}x{}x{}", t.width, t.height, t.layers);
    assert!(t.rgba_layer(t.layers - 1).is_ok());
}

#[test]
fn a_figure_carries_colours_tinted_parts_and_face_textures() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let f = crate::figure::build(&avd, &d).unwrap();
    assert_eq!(f.parts.len(), 5);
    assert_eq!(f.face.tones.skin, crate::shade::rgb(0xFFD7AA71));
    assert_eq!(f.face.tones.hair, crate::shade::rgb(0xFFE3802F));
    assert!(f.face.mouth.is_some() && f.face.eye.is_some() && f.face.eyebrow.is_some());
    assert!(f.face.facial_hair.is_none());
    assert!(f.parts.iter().filter(|p| p.custom.is_some()).count() >= 2);
    let image = f.preview(256, 256);
    assert!(image.rgba.chunks(4).filter(|c| c[3] > 0).count() > 3000);
}

#[test]
fn pack_assets_have_names_and_standing_animations_pose_the_figure() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    assert_eq!(toc::name(&d, 0).as_deref(), Some("MaleBody"));
    assert_eq!(
        toc::name(&d, 3).as_deref(),
        Some("Animation Generic Stand 0")
    );
    let rotations = crate::figure::animation_rotations(&d, 3, 0).unwrap();
    assert_eq!(rotations.len(), 71);
    let f = crate::figure::build(&avd, &d).unwrap();
    let rest = f.preview(192, 192);
    let posed = f.posed(&rotations).unwrap().preview(192, 192);
    assert_ne!(rest.rgba, posed.rgba);
    let width = |p: &crate::inspect::Preview| {
        let cols: Vec<usize> = p
            .rgba
            .chunks(4)
            .enumerate()
            .filter(|(_, c)| c[3] > 0)
            .map(|(i, _)| i % p.width as usize)
            .collect();
        cols.iter().max().unwrap() - cols.iter().min().unwrap()
    };
    assert!(
        width(&posed) < width(&rest),
        "{} {}",
        width(&posed),
        width(&rest)
    );
}

#[test]
fn hiding_templates_list_the_body_triangles_a_garment_covers() {
    let d = pack();
    let t = crate::hiding::template(toc::entry(&d, 1033).unwrap().data).unwrap();
    assert_eq!(t.target_index_bytes, 13584);
    assert_eq!(&t.triangles[..8], &[97, 98, 99, 100, 101, 102, 149, 150]);
    assert_eq!(t.triangles.len(), 328);
    assert!(t.triangles.windows(2).all(|w| w[0] < w[1]));
    let blend = crate::hiding::template(toc::entry(&d, 797).unwrap().data).unwrap();
    assert!(blend.triangles.is_empty());
    assert_eq!(blend.target_index_bytes, 5076);
}

#[test]
fn garments_find_their_hiding_template_by_name_and_hide_body_triangles() {
    let d = pack();
    assert_eq!(toc::hiding_template_for(&d, 369), Some(887));
    assert_eq!(toc::hiding_template_for(&d, 0), None);
    assert_eq!(toc::hiding_template_for(&d, 257), None);
    let body = crate::body::decode(toc::entry(&d, 0).unwrap().data).unwrap();
    let t = crate::hiding::template(toc::entry(&d, 1033).unwrap().data).unwrap();
    let covered = crate::hiding::apply(&body, &t.triangles);
    assert_eq!(
        covered.batches[0].triangles as usize,
        body.batches[0].triangles as usize - t.triangles.len()
    );
    assert_eq!(
        covered.batches[0].indices.len(),
        covered.batches[0].triangles as usize * 3
    );
}

#[test]
fn weight_and_height_factors_reshape_the_figure() {
    let d = pack();
    let mut avd = std::fs::read(FAKEREAL).unwrap();
    let f = crate::figure::build(&avd, &d).unwrap();
    assert_eq!((f.weight, f.height), (-0.5, -1.0));
    let top = |f: &crate::figure::Figure| {
        f.parts
            .iter()
            .flat_map(|p| {
                p.model
                    .batches
                    .iter()
                    .flat_map(|b| b.vertices.iter().map(|v| v.position[1]))
            })
            .fold(f32::MIN, f32::max)
    };
    let shaped = f.posed(&[]).unwrap();
    avd[4..8].copy_from_slice(&0.0f32.to_be_bytes());
    avd[8..12].copy_from_slice(&1.0f32.to_be_bytes());
    let tall = crate::figure::build(&avd, &d).unwrap().posed(&[]).unwrap();
    assert!(
        top(&shaped) < top(&f) && top(&tall) > top(&f),
        "{} {} {}",
        top(&shaped),
        top(&f),
        top(&tall)
    );
}

#[test]
fn a_standing_figure_uses_the_generic_stand_animation() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    assert_eq!(toc::find_by_name(&d, "Animation Generic Stand 0"), Some(3));
    let f = crate::figure::build(&avd, &d).unwrap();
    let standing = f.standing(&d).unwrap();
    assert_ne!(standing.preview(128, 128).rgba, f.preview(128, 128).rgba);
}

#[test]
fn every_hiding_template_is_a_sorted_triangle_list_within_its_target() {
    let d = pack();
    let mut templates = 0;
    for a in toc::split(&d).iter().filter(|a| a.asset_type == 3) {
        let t = crate::hiding::template(a.data).unwrap();
        let limit = t.target_index_bytes / 6;
        assert!(
            t.triangles.windows(2).all(|w| w[0] < w[1]) && t.triangles.iter().all(|&v| v < limit),
            "asset {}",
            a.id
        );
        templates += usize::from(!t.triangles.is_empty());
    }
    assert!(templates > 150, "{templates}");
}

#[test]
fn the_browser_opens_a_toc_file_as_a_folder_of_named_assets() {
    let _ = pack();
    let mut b = crate::browser::Browser::new(&PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/avatar"
    )))
    .unwrap();
    let at = b
        .entries()
        .iter()
        .position(|e| e.name == "AvatarAssetPack.toc")
        .unwrap();
    b.open(at).unwrap();
    assert_eq!(b.entries().len(), 1102);
    assert_eq!(b.entries()[0].name, "0000 MaleBody.type1.avatarasset");
    let det = crate::inspect::describe("AvatarAssetPack.toc", &pack());
    assert_eq!(det.kind, "Avatar asset pack");
    assert!(
        det.lines.iter().any(|l| l.starts_with("1102 assets")),
        "{:?}",
        det.lines
    );
}

#[test]
fn details_carry_a_scene_that_reproduces_their_preview() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let det = crate::avatar::describe_avd_with(&avd, Some(&d)).unwrap();
    let scene = det.scene.as_ref().unwrap();
    assert_eq!(
        scene
            .0
            .render(&crate::render::Camera::default(), 384, 384)
            .rgba,
        det.image.as_ref().unwrap().rgba
    );
    let asset = toc::entry(&d, 49).unwrap();
    let packed = crate::inspect::describe("0049 x.type1.avatarasset", asset.data);
    let scene = packed.scene.as_ref().unwrap();
    assert_eq!(
        scene
            .0
            .render(&crate::render::Camera::default(), 384, 384)
            .rgba,
        packed.image.as_ref().unwrap().rgba
    );
    assert!(crate::inspect::describe("a.txt", b"hello").scene.is_none());
}

#[test]
fn expressions_pick_the_matching_layer_of_each_animated_face_texture() {
    use crate::figure::Expression;
    assert_eq!(Expression::Neutral.layers(), (0, 0, 0));
    assert_eq!(Expression::Happy.layers(), (6, 6, 0));
    assert_eq!(Expression::Shocked.layers(), (5, 5, 4));
    assert_eq!(Expression::ALL.len(), 7);
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let f = crate::figure::build(&avd, &d).unwrap();
    let happy = f.express(Expression::Happy);
    assert_ne!(
        happy.face.mouth.as_ref().unwrap().rgba,
        f.face.mouth.as_ref().unwrap().rgba
    );
    assert_ne!(happy.preview(160, 160).rgba, f.preview(160, 160).rgba);
    assert_eq!(
        f.express(Expression::Neutral)
            .face
            .mouth
            .as_ref()
            .unwrap()
            .rgba,
        f.face.mouth.as_ref().unwrap().rgba
    );
}

#[test]
fn the_pack_lists_its_poses_by_name() {
    let d = pack();
    let poses = crate::figure::poses(&d);
    assert_eq!(poses.len(), 60);
    assert!(poses.contains(&(3, "Animation Generic Stand 0".to_owned())));
}

#[test]
fn a_styled_scene_applies_pose_and_expression_and_rest_means_t_pose() {
    use crate::figure::Expression;
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let f = crate::figure::build(&avd, &d).unwrap();
    let render =
        |s: crate::render::Scene| s.render(&crate::render::Camera::default(), 128, 128).rgba;
    let tpose = render(f.styled(&d, None, Expression::Neutral).unwrap());
    let stand = render(f.styled(&d, Some(3), Expression::Neutral).unwrap());
    let smile = render(f.styled(&d, Some(3), Expression::Happy).unwrap());
    assert_ne!(tpose, stand);
    assert_ne!(stand, smile);
    assert!(f.styled(&d, Some(0), Expression::Neutral).is_err());
}

#[test]
fn the_figure_a_detail_carries_is_unposed_so_later_poses_do_not_stack() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let det = crate::avatar::describe_avd_with(&avd, Some(&d)).unwrap();
    let figure = det.figure.unwrap();
    let again = figure
        .0
        .styled(&d, Some(3), crate::figure::Expression::Neutral)
        .unwrap();
    let camera = crate::render::Camera::default();
    assert_eq!(
        again.render(&camera, 128, 128).rgba,
        det.scene.unwrap().0.render(&camera, 128, 128).rgba
    );
}

#[test]
fn garments_without_a_named_template_get_the_best_fitting_one() {
    let d = pack();
    let female = crate::body::decode(toc::entry(&d, 1).unwrap().data).unwrap();
    let heels = crate::body::decode(toc::entry(&d, 257).unwrap().data).unwrap();
    assert_eq!(toc::hiding_template_for(&d, 257), None);
    let templates = crate::hiding::all_templates(&d);
    assert!(templates.len() > 150);
    let fit = crate::hiding::best_fit(&female, &heels, &templates).unwrap();
    let b = &female.batches[0];
    let top = fit
        .triangles
        .iter()
        .map(|&t| {
            (0..3)
                .map(|i| b.vertices[usize::from(b.indices[t as usize * 3 + i])].position[1])
                .sum::<f32>()
                / 3.0
        })
        .fold(f32::MIN, f32::max);
    assert!(top < 0.5, "hides up to {top}");
    let shirt = crate::body::decode(toc::entry(&d, 288).unwrap().data).unwrap();
    let torso = crate::hiding::best_fit(&female, &shirt, &templates).unwrap();
    assert!(torso.triangles.len() > 100);
    assert!(crate::hiding::best_fit(
        &female,
        &crate::body::decode(toc::entry(&d, 1).unwrap().data).unwrap(),
        &[]
    )
    .is_none());
}

#[test]
fn blend_shapes_list_the_head_vertices_they_move() {
    let d = pack();
    let head = crate::body::decode(toc::entry(&d, 2).unwrap().data).unwrap();
    for (id, count) in [(827usize, 38usize), (797, 96), (809, 116)] {
        let shape = crate::blend::shape(toc::entry(&d, id).unwrap().data).unwrap();
        assert_eq!(shape.entries.len(), count, "asset {id}");
        let moved = crate::blend::apply(&head, &shape);
        let changed = head
            .batches
            .iter()
            .zip(&moved.batches)
            .flat_map(|(a, b)| a.vertices.iter().zip(&b.vertices))
            .filter(|(a, b)| a.position != b.position)
            .count();
        assert!(
            changed >= count - 2 && changed <= count,
            "{id}: {changed} of {count}"
        );
        let worst = shape
            .entries
            .iter()
            .map(|e| {
                let (b, i) = shape.locate(&head, e).unwrap();
                let v = &head.batches[b].vertices[i];
                (0..3)
                    .map(|k| (e.position[k] - v.position[k]).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .fold(0.0f32, f32::max);
        assert!(worst < 0.04, "asset {id} moves a vertex by {worst}");
    }
    assert_eq!(
        crate::blend::shape(toc::entry(&d, 827).unwrap().data)
            .unwrap()
            .entries[0]
            .offset,
        622 * 52
    );
    assert!(crate::blend::shape(toc::entry(&d, 0).unwrap().data).is_err());
}

#[test]
fn a_figure_applies_its_blend_shapes_to_the_head() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let f = crate::figure::build(&avd, &d).unwrap();
    let head = f.parts.iter().find(|p| p.component == 1).unwrap();
    let plain = crate::body::decode(toc::entry(&d, 2).unwrap().data).unwrap();
    let changed = head
        .model
        .batches
        .iter()
        .zip(&plain.batches)
        .flat_map(|(a, b)| a.vertices.iter().zip(&b.vertices))
        .filter(|(a, b)| a.position != b.position)
        .count();
    assert!(changed > 50, "{changed}");
}

#[test]
fn only_the_first_record_list_is_worn_the_second_is_an_alternate_outfit() {
    let avd = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/common/v1.0.808.0_debug_unmodified/media/avatars/FemaleHost.avd"
    ))
    .unwrap();
    assert_eq!(
        crate::avatar::worn_ids(&avd),
        vec![1, 2, 424, 257, 369, 545]
    );
}

#[test]
fn the_body_is_pushed_back_in_depth_so_garments_never_let_it_poke_through() {
    let d = pack();
    let avd = std::fs::read(FAKEREAL).unwrap();
    let scene = crate::figure::build(&avd, &d).unwrap().scene();
    let biased: Vec<bool> = scene.parts.iter().map(|p| p.depth_bias > 0.0).collect();
    assert_eq!(biased.iter().filter(|b| **b).count(), 1);
    assert!(
        scene
            .parts
            .iter()
            .find(|p| p.depth_bias > 0.0)
            .unwrap()
            .model
            .batches[0]
            .triangles
            > 2000
    );
}
