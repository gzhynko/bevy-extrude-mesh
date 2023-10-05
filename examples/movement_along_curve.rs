use bevy::gltf::{Gltf, GltfMesh};
use bevy::pbr::wireframe::{Wireframe, WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_flycam::{FlyCam, NoCameraPlayerPlugin};
use bevy_egui::{egui, EguiContext, EguiContexts, EguiPlugin};
use bevy::prelude::shape::UVSphere;
use bevy::render::mesh::MeshVertexAttribute;

use bevy_extrude_mesh::extrude;
use bevy_extrude_mesh::extrude::ExtrudeShape;
use bevy_extrude_mesh::bezier::{BezierCurve};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(NoCameraPlayerPlugin)

        .add_plugins(EguiPlugin)
        .add_plugins(WireframePlugin)

        .add_state::<GameState>()
        .add_loading_state(
            LoadingState::new(GameState::AssetsLoading)
                .continue_to_state(GameState::AssetsLoaded)
        )
        .add_collection_to_loading_state::<_, TrackAssets>(GameState::AssetsLoading)

        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::AssetsLoaded), spawn_extruded_mesh)
        .add_systems(Update, update_movement_path)
        .add_systems(Update, ui)

        .insert_resource(MovementParameters::default())

        .run();
}

#[derive(AssetCollection, Resource)]
struct TrackAssets {
    #[asset(path = "models/test_shape.glb")]
    track: Handle<Gltf>,
}

#[derive(Resource)]
struct ExtrudedMeshHandle(Handle<Mesh>);
#[derive(Component)]
struct MovementStepPointMarker;

#[derive(Resource)]
struct MovementParameters {
    curve: Option<BezierCurve>,
    num_steps: u32,
    space_uniformly: bool,
    is_dirty: bool,
}

impl Default for MovementParameters {
    fn default() -> Self {
        Self {
            curve: None,
            num_steps: 20,
            space_uniformly: false,
            is_dirty: true,
        }
    }
}

fn setup(
    mut commands: Commands,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    // point light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // camera
    commands
        .spawn(Camera3dBundle {
            ..default()
        })
        .insert(FlyCam);

    wireframe_config.global = true;
}

fn spawn_extruded_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params: ResMut<MovementParameters>,

    assets: Res<TrackAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_mesh_assets: Res<Assets<GltfMesh>>,
) {
    if let Some(gltf) = gltf_assets.get(&assets.track) {
        let track_gltf_mesh = gltf_mesh_assets.get(&gltf.named_meshes["TestShape"]).unwrap();
        let track_mesh = meshes.get(&track_gltf_mesh.primitives[0].mesh).unwrap();
        let extrude_shape = ExtrudeShape::from_mesh(track_mesh);

        let control_points = vec![
            Vec3::new(0., 0., 0.,),
            Vec3::new(0., 0., 10.,),
            Vec3::new(0., 10., 0.,),
            Vec3::new(0., 10., 10.,),
        ];
        let mut bezier = BezierCurve::new(control_points, None);
        bezier.calculate_arc_lengths();
        let path = bezier.generate_path(params.num_steps);
        params.curve = Some(bezier);

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

fn update_movement_path(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params: ResMut<MovementParameters>,
    step_markers: Query<Entity, With<MovementStepPointMarker>>,
) {
    if !params.is_dirty || params.curve.is_none() {
        return;
    }

    // Remove old objects
    for entity in &step_markers {
        commands.entity(entity).despawn();
    }

    let mesh = meshes.add(Mesh::from(UVSphere { radius: 0.1, ..default() }));
    let red_material = materials.add(StandardMaterial { base_color: Color::RED, unlit: true, ..default() });

    let step = 1. / params.num_steps as f32;
    let bezier_curve = params.curve.as_ref().unwrap();
    let mut t = 0.;
    while t <= 1. {
        let point;
        if params.space_uniformly {
            let actual_t = bezier_curve.map(t);
            point = bezier_curve.get_oriented_point(actual_t);
        } else {
            point = bezier_curve.get_oriented_point(t);
        }

        commands.spawn(PbrBundle {
            mesh: mesh.clone(),
            material: red_material.clone(),
            transform: Transform::from_translation(point.position),
            ..default()
        })
            .insert(MovementStepPointMarker);

        t += step;
    }

    params.is_dirty = false;
}

fn ui(
    mut contexts: EguiContexts,
    mut params: ResMut<MovementParameters>,
) {
    egui::Window::new("Parameters").show(contexts.ctx_mut(), |ui| {
        let old_subdivisions = params.num_steps;
        ui.add(egui::Slider::new(&mut params.num_steps, 10u32..=100u32).text("Number of steps"));

        let old_space_uniformly = params.space_uniformly;
        ui.checkbox(&mut params.space_uniformly, "Space steps uniformly");

        if old_subdivisions != params.num_steps || old_space_uniformly != params.space_uniformly {
            params.is_dirty = true; // mark for update
        }
    });
}

#[derive(Default, Clone, Eq, PartialEq, Debug, Hash, States)]
enum GameState {
    #[default]
    AssetsLoading,
    AssetsLoaded,
}
