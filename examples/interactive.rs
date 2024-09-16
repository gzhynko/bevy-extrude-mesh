use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_flycam::{FlyCam, NoCameraPlayerPlugin};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use bevy_extrude_mesh::extrude;
use bevy_extrude_mesh::extrude::ExtrudeShape;
use bevy_extrude_mesh::bezier::{BezierCurve};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(NoCameraPlayerPlugin)

        .add_plugins(EguiPlugin)
        .add_plugins(bevy_mod_picking::DefaultPickingPlugins)
        .add_plugins(bevy_transform_gizmo::TransformGizmoPlugin::default())

        .init_state::<GameState>()
        .add_loading_state(
            LoadingState::new(GameState::AssetsLoading)
                .continue_to_state(GameState::AssetsLoaded)
        )
        .add_collection_to_loading_state::<_, TrackAssets>(GameState::AssetsLoading)

        .add_systems(Startup, setup)
        .add_systems(Startup, spawn_control_point_spheres)
        .add_systems(OnEnter(GameState::AssetsLoaded), spawn_extruded_mesh)
        .add_systems(Update, properties_ui)
        .add_systems(Update, update_extruded_mesh)

        .insert_resource(MeshExtrusionParameters::default())

        .run();
}

#[derive(AssetCollection, Resource)]
struct TrackAssets {
    #[asset(path = "models/track.glb")]
    track: Handle<Gltf>,
}

#[derive(Resource)]
struct ExtrudedMeshHandle(Handle<Mesh>);
#[derive(Component)]
struct BezierControlGizmoSphereMarker(usize);

#[derive(Resource)]
struct MeshExtrusionParameters {
    control_points: Vec<Vec3>,
    old_control_points: Vec<Vec3>,
    subdivisions: u32,
    is_dirty: bool,
}

impl Default for MeshExtrusionParameters {
    fn default() -> Self {
        let control_points = vec![
            Vec3::new(0., 0., 0.,),
            Vec3::new(0., 0., 10.,),
            Vec3::new(0., 10., 0.,),
            Vec3::new(0., 10., 10.,),
        ];
        Self {
            old_control_points: control_points.clone(),
            control_points,
            subdivisions: 15,
            is_dirty: false,
        }
    }
}

fn setup(
    mut commands: Commands,
) {
    // point light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 10.0, 0.0),
        ..default()
    });

    // camera
    commands
        .spawn(Camera3dBundle {
            ..default()
        })
        .insert(FlyCam)
        .insert(bevy_transform_gizmo::GizmoPickSource::default());
}

fn spawn_extruded_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    params: Res<MeshExtrusionParameters>,

    assets: Res<TrackAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_mesh_assets: Res<Assets<GltfMesh>>,
) {
    if let Some(gltf) = gltf_assets.get(&assets.track) {
        let track_gltf_mesh = gltf_mesh_assets.get(&gltf.named_meshes["TrackCrossSection"]).unwrap();
        let track_mesh = meshes.get(&track_gltf_mesh.primitives[0].mesh).unwrap();
        let extrude_shape = ExtrudeShape::from_mesh(track_mesh);

        let bezier = BezierCurve::new(params.control_points.clone(), None);
        let path = bezier.generate_path(params.subdivisions);

        let extruded = extrude::extrude(&extrude_shape, &path);
        let extruded_handle = meshes.add(extruded);

        commands.insert_resource(ExtrudedMeshHandle(extruded_handle.clone()));
        commands.spawn(PbrBundle {
            mesh: extruded_handle,
            material: materials.add(StandardMaterial::default()),
            ..default()
        });
    }
}

fn spawn_control_point_spheres(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    params: Res<MeshExtrusionParameters>,
) {
    let mesh = meshes.add(Mesh::from(Sphere { radius: 0.2, ..default() }));

    let red_material = materials.add(StandardMaterial { base_color: Color::linear_rgb(1.0, 0.0, 0.0), unlit: true, ..default() });
    let gray_material = materials.add(StandardMaterial { base_color: Color::linear_rgba(0.0, 0.0, 0.0, 0.5), unlit: true, ..default() });

    for i in 0..params.control_points.len() {
        let pos = params.control_points[i];
        commands
            .spawn(PbrBundle {
                mesh: mesh.clone(),
                material: if i == 0 || i == params.control_points.len() - 1 { red_material.clone() } else { gray_material.clone() },
                transform: Transform::from_translation(pos),
                ..default()
            })
            .insert(BezierControlGizmoSphereMarker(i))
            .insert(bevy_mod_picking::PickableBundle::default())
            .insert(bevy_transform_gizmo::GizmoTransformable::default());
    }
}

fn update_extruded_mesh(
    mut meshes: ResMut<Assets<Mesh>>,
    mut params: ResMut<MeshExtrusionParameters>,
    gizmo_spheres_query: Query<(&Transform, &BezierControlGizmoSphereMarker)>,
    extruded_mesh_handle: Option<Res<ExtrudedMeshHandle>>,

    assets: Option<Res<TrackAssets>>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_mesh_assets: Res<Assets<GltfMesh>>,
) {
    if extruded_mesh_handle.is_none() || assets.is_none() {
        return;
    }

    if let Some(gltf) = gltf_assets.get(&assets.unwrap().track) {
        let track_gltf_mesh = gltf_mesh_assets.get(&gltf.named_meshes["TrackCrossSection"]).unwrap();
        let track_mesh = meshes.get(&track_gltf_mesh.primitives[0].mesh).unwrap();

        let mut new_sphere_positions = vec![Vec3::ZERO; 4];
        for (transform, marker) in gizmo_spheres_query.iter() {
            new_sphere_positions[marker.0] = transform.translation;
        }

        if new_sphere_positions != params.old_control_points || params.is_dirty {
            let extrude_shape = ExtrudeShape::from_mesh(track_mesh);
            let mut mesh = meshes.get_mut(&extruded_mesh_handle.unwrap().0).unwrap();

            let bezier = BezierCurve::new(new_sphere_positions.clone(), None);
            let path = bezier.generate_path(params.subdivisions);
            let extruded = extrude::extrude(&extrude_shape, &path);
            modify_existing_mesh(&mut mesh, extruded);

            params.is_dirty = false;
            params.old_control_points = new_sphere_positions;
        }
    }
}

fn properties_ui(
    mut contexts: EguiContexts,
    mut params: ResMut<MeshExtrusionParameters>,
) {
    egui::Window::new("Properties").show(contexts.ctx_mut(), |ui| {
        // Subdivisions slider
        let old_subdivisions = params.subdivisions;
        ui.add(egui::Slider::new(&mut params.subdivisions, 1u32..=100u32).text("Number of subdivisions"));
        if old_subdivisions != params.subdivisions {
            params.is_dirty = true; // mark for mesh update
        }
    });
}

fn modify_existing_mesh(mesh: &mut Mesh, extruded: Mesh) {
    mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION);
    mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL);
    mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0);

    let new_indices = extruded.indices().unwrap().clone();
    let new_vertices = extruded.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
    let new_normals = extruded.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().as_float3().unwrap();
    let new_uvs = extruded.attribute(Mesh::ATTRIBUTE_UV_0);

    mesh.insert_indices(new_indices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, new_vertices.to_owned());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, new_normals.to_owned());
    if new_uvs.is_some() {
        if let Some(uv_array) = new_uvs.unwrap().as_float3() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv_array.to_owned());
        }
    }
}

#[derive(Default, Clone, Eq, PartialEq, Debug, Hash, States)]
enum GameState {
    #[default]
    AssetsLoading,
    AssetsLoaded,
}
