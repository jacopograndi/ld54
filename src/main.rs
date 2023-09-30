use std::{f32::consts::PI, time::Duration};

use bevy::{
    app::AppExit,
    asset::LoadState,
    audio::VolumeLevel,
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*,
    utils::HashMap,
    window::PrimaryWindow,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                fit_canvas_to_parent: true,
                resolution: (1200., 720.).into(),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_state::<AppState>()
        .add_systems(Startup, (startup).chain())
        .add_systems(Update, check_loading.run_if(in_state(AppState::Loading)))
        .add_systems(OnExit(AppState::Loading), gen_atlas)
        .add_systems(OnExit(AppState::Loading), play_song)
        .add_systems(OnEnter(AppState::Setup), (setup_scene).chain())
        .add_systems(Update, escape_exit)
        .add_systems(
            Update,
            (
                turn,
                ship_orbit,
                on_build_construction,
                on_destroy_construction,
                on_modify_resource,
            )
                .chain()
                .run_if(in_state(AppState::Gameplay)),
        )
        .add_systems(Update, send_end_turn.run_if(in_state(AppState::Gameplay)))
        .add_systems(
            Update,
            play_autoactions.run_if(in_state(AppState::Gameplay)),
        )
        .add_systems(
            Update,
            (
                highlight,
                ui_on_node_selected_constr,
                ui_on_node_selected_move,
                ui_on_node_selected_planet,
                ui_on_construction,
                button_system,
            )
                .run_if(in_state(AppState::Gameplay)),
        )
        .add_systems(
            Update,
            (interpolation_fx, on_modify_resource_fx).run_if(in_state(AppState::Gameplay)),
        )
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(AssetHandles::default())
        .insert_resource(Map::test())
        .insert_resource(AutoActions::default())
        .add_event::<EndTurn>()
        .add_event::<BuildConstruction>()
        .add_event::<DestroyConstruction>()
        .add_event::<ModifyResource>()
        .add_event::<ModifyResourceFx>()
        .add_event::<UiEvent>()
        .run();
}

fn escape_exit(keys: Res<Input<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if keys.pressed(KeyCode::Escape) {
        exit.send(AppExit);
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Setup,
    Gameplay,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct AssetHandles {
    sheet: Handle<Image>,
    atlas: Handle<TextureAtlas>,
    font: Handle<Font>,
    song: Handle<AudioSource>,
    ship: Handle<Image>,
}

fn startup(
    mut commands: Commands,
    mut handles: ResMut<AssetHandles>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2dBundle::default());
    handles.sheet = asset_server.load("sheet.png");
    handles.font = asset_server.load("FFFFORWA.TTF");
    handles.song = asset_server.load("song.ogg");
    handles.ship = asset_server.load("ship.png");
}

fn check_loading(
    handles: Res<AssetHandles>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let mut loaded = true;
    loaded &= matches!(
        asset_server.get_load_state(handles.sheet.clone()),
        LoadState::Loaded
    );
    loaded &= matches!(
        asset_server.get_load_state(handles.song.clone()),
        LoadState::Loaded
    );
    loaded &= matches!(
        asset_server.get_load_state(handles.ship.clone()),
        LoadState::Loaded
    );
    if loaded {
        next_state.set(AppState::Setup);
    }
}

#[derive(Component)]
struct Song;
fn play_song(mut commands: Commands, handles: Res<AssetHandles>) {
    commands.spawn((
        AudioBundle {
            source: handles.song.clone(),
            settings: PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: bevy::audio::Volume::Relative(VolumeLevel::new(0.2)),
                ..Default::default()
            },
        },
        Song,
    ));
}

const TILE_SIZE: Vec2 = Vec2 { x: 64.0, y: 64.0 };

fn gen_atlas(mut texture_atlases: ResMut<Assets<TextureAtlas>>, mut handles: ResMut<AssetHandles>) {
    let texture_handle = handles.sheet.clone();
    let texture_atlas = TextureAtlas::from_grid(texture_handle, TILE_SIZE, 8, 8, None, None);
    handles.atlas = texture_atlases.add(texture_atlas);
}

/// Map
#[derive(Clone, Debug, Resource)]
struct Map {
    nodes: Vec<NodeId>,
    groups: HashMap<GroupId, Vec<NodeId>>,
    edges: Vec<(GroupId, GroupId)>,
    positions: HashMap<NodeId, Vec2>,
    group_positions: HashMap<GroupId, Vec2>,
    occupation: HashMap<NodeId, NodeOccupant>,
}

#[derive(Debug, Clone, Deref, DerefMut, PartialEq, Eq, Hash)]
struct NodeId(usize);
#[derive(Debug, Clone, Deref, DerefMut, PartialEq, Eq, Hash)]
struct GroupId(usize);

#[derive(Debug, Clone)]
enum NodeOccupant {
    Construction {
        var: ConstructionVariant,
        cooldown: u32,
    },
    Stockpile {
        var: ResourceVariant,
        amt: u32,
    },
}

const MAX_STOCKPILE: u32 = 100;

impl Map {
    fn test() -> Self {
        let nodes: Vec<NodeId> = (0..25).map(|i| NodeId(i)).collect();
        let mut map = Self {
            nodes: nodes.clone(),
            groups: HashMap::from([
                (GroupId(0), (0..5).map(|i| NodeId(i)).collect()),
                (GroupId(1), (5..10).map(|i| NodeId(i)).collect()),
                (GroupId(2), (10..20).map(|i| NodeId(i)).collect()),
                (GroupId(3), (20..23).map(|i| NodeId(i)).collect()),
                (GroupId(4), (23..25).map(|i| NodeId(i)).collect()),
            ]),
            edges: vec![
                (GroupId(0), GroupId(1)),
                (GroupId(1), GroupId(2)),
                (GroupId(4), GroupId(0)),
            ],
            positions: HashMap::from([
                (NodeId(0), Vec2::new(0., 0.)),
                (NodeId(1), Vec2::new(64., 0.)),
                (NodeId(2), Vec2::new(128., 0.)),
                (NodeId(3), Vec2::new(-64., 0.)),
                (NodeId(4), Vec2::new(-128., 0.)),
                (NodeId(5), Vec2::new(0., 128.)),
                (NodeId(6), Vec2::new(64., 128.)),
                (NodeId(7), Vec2::new(128., 128.)),
                (NodeId(8), Vec2::new(-64., 128.)),
                (NodeId(9), Vec2::new(-128., 128.)),
                (NodeId(10), Vec2::new(0., -128.)),
                (NodeId(11), Vec2::new(64., -128.)),
                (NodeId(12), Vec2::new(128., -128.)),
                (NodeId(13), Vec2::new(-64., -128.)),
                (NodeId(14), Vec2::new(-128., -128.)),
                (NodeId(15), Vec2::new(0., -192.)),
                (NodeId(16), Vec2::new(64., -192.)),
                (NodeId(17), Vec2::new(128., -192.)),
                (NodeId(18), Vec2::new(-64., -192.)),
                (NodeId(19), Vec2::new(-128., -192.)),
                (NodeId(20), Vec2::new(0., -320.)),
                (NodeId(21), Vec2::new(64., -320.)),
                (NodeId(22), Vec2::new(128., -320.)),
                (NodeId(23), Vec2::new(0., 320.)),
                (NodeId(24), Vec2::new(64., 320.)),
            ]),
            group_positions: HashMap::from([
                (GroupId(0), Vec2::new(-256., 0.)),
                (GroupId(1), Vec2::new(-256., 128.)),
                (GroupId(2), Vec2::new(-256., -128.)),
                (GroupId(3), Vec2::new(-256., -320.)),
                (GroupId(4), Vec2::new(-256., 320.)),
            ]),
            occupation: HashMap::default(),
        };
        map
    }

    fn star(&self, group_id: &GroupId) -> Vec<GroupId> {
        self.edges
            .iter()
            .filter_map(|edge| match edge {
                (n, m) if n == group_id => Some(m.clone()),
                (m, n) if n == group_id => Some(m.clone()),
                _ => None,
            })
            .collect()
    }

    fn group_from_node(&self, id: &NodeId) -> GroupId {
        self.groups
            .iter()
            .find(|(_, ids)| ids.contains(id))
            .expect("no group")
            .0
            .clone()
    }

    fn set_at(&mut self, id: &NodeId, occ: NodeOccupant) {
        self.occupation.insert(id.clone(), occ);
    }

    fn get_group_bunch(&self, id: &GroupId) -> Bunch {
        let group = self.groups.get(id).expect("no group");
        group
            .iter()
            .filter_map(|node_id| match self.occupation.get(node_id) {
                Some(NodeOccupant::Stockpile { var, amt }) => {
                    Some(Bunch::single(var.clone(), *amt))
                }
                _ => None,
            })
            .sum()
    }

    fn get_lowest_stockpile(&self, id: &GroupId, v: &ResourceVariant) -> NodeId {
        #[cfg(feature = "dbtrace")]
        println!("getting lowest stockpile for {:?} {:?}", id, v);
        let group = self.groups.get(id).expect("no group");
        let node_id = group
            .iter()
            .filter_map(|node_id| match self.occupation.get(node_id) {
                Some(NodeOccupant::Stockpile { var, amt }) if v == var => Some((node_id, amt)),
                _ => None,
            })
            .min_by(|a, b| a.1.cmp(&b.1))
            .expect("no stockpile")
            .0
            .clone();
        #[cfg(feature = "dbtrace")]
        println!("found lowest stockpile at {:?}", node_id);
        node_id
    }

    /// add to highest stockpile until amt = 0 or put in empty.
    fn add_resource_in_group(
        &mut self,
        group_id: &GroupId,
        v: &ResourceVariant,
        amt: u32,
    ) -> Result<Vec<(NodeId, u32, i32)>, String> {
        #[cfg(feature = "dbtrace")]
        println!("adding to {:?} {:?} {:?}", group_id, v, amt);
        assert!(amt <= 100);
        let group = self.groups.get(group_id).expect("no group").clone();
        let mut left = amt;
        let mut actions = vec![];
        for _i in 0..16 {
            if left <= 0 {
                break;
            }
            // is there already a pile?
            if let Some(node_id) = group
                .iter()
                .filter_map(|node_id| match self.occupation.get(node_id) {
                    Some(NodeOccupant::Stockpile { var, amt })
                        if (v == var && *amt < MAX_STOCKPILE) =>
                    {
                        Some((node_id, *amt))
                    }
                    _ => None,
                })
                .max_by(|a, b| a.1.cmp(&b.1))
            {
                // insert into highest
                let highest = self.occupation.get_mut(node_id.0).unwrap();
                match highest {
                    NodeOccupant::Stockpile { amt: stock_amt, .. } => {
                        let clamped = left.min(MAX_STOCKPILE - *stock_amt);
                        actions.push((node_id.0.clone(), clamped + *stock_amt, clamped as i32));
                        *stock_amt += clamped;
                        left -= clamped;
                    }
                    _ => unreachable!(),
                }
            } else {
                // insert into an eventual empty tile
                if let Some(empty_id) = group
                    .iter()
                    .find(|node_id| self.occupation.get(*node_id).is_none())
                {
                    actions.push((empty_id.clone(), left, left as i32));
                    self.set_at(
                        empty_id,
                        NodeOccupant::Stockpile {
                            var: v.clone(),
                            amt: left,
                        },
                    )
                }
                left = 0;
            };
        }
        Ok(actions)
    }
}

// events
#[derive(Event)]
struct EndTurn;

#[derive(Event)]
struct BuildConstruction {
    node_id: NodeId,
    var: ConstructionVariant,
}

#[derive(Event)]
struct DestroyConstruction {
    node_id: NodeId,
}

#[derive(Event)]
struct ModifyResource {
    node_id: NodeId,
    var: ResourceVariant,
    abs: u32,
}

#[derive(Event)]
struct ModifyResourceFx {
    from: NodeId,
    to: NodeId,
    var: ResourceVariant,
    diff: i32,
}

#[derive(Event)]
enum UiEvent {
    SelectNodeForConstruction(NodeId),
    ConstructOnNode(NodeId),
    SelectNodeForMove(NodeId),
    SelectPlanet(GroupId),
    Close,
}

#[derive(Event)]
struct NodeConstruct {
    node_id: NodeId,
}

fn on_destroy_construction(
    mut events: EventReader<DestroyConstruction>,
    mut commands: Commands,
    query: Query<(Entity, &NodeIdMarker)>,
) {
    for event in events.iter() {
        for (e, marker) in query.iter() {
            if marker.node_id == event.node_id {
                commands.entity(e).despawn_recursive();
            }
        }
    }
}

fn on_build_construction(
    mut events: EventReader<BuildConstruction>,
    mut commands: Commands,
    map: Res<Map>,
    handles: Res<AssetHandles>,
) {
    for event in events.iter() {
        let pos = map.positions.get(&event.node_id).unwrap().extend(1.);
        commands.spawn((
            NodeIdMarker {
                node_id: event.node_id.clone(),
            },
            SpriteSheetBundle {
                transform: Transform::default().with_translation(pos),
                sprite: TextureAtlasSprite {
                    index: event.var.get_sprite_index(),
                    ..Default::default()
                },
                texture_atlas: handles.atlas.clone(),
                ..Default::default()
            },
        ));
    }
}

fn on_modify_resource(
    mut commands: Commands,
    mut events: EventReader<ModifyResource>,
    mut stock_q: Query<(Entity, &NodeIdMarker, &Children)>,
    mut text_q: Query<&mut Text>,
    map: Res<Map>,
    handles: Res<AssetHandles>,
) {
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::BLACK,
    };
    let text_alignment = TextAlignment::Center;

    for event in events.iter() {
        if let Some((ent, _, children)) = stock_q
            .iter_mut()
            .find(|(_, marker, _)| marker.node_id == event.node_id)
        {
            if event.abs > 0 {
                let mut text = text_q.get_mut(children[0]).unwrap();
                text.sections[0].value = format!("{}", event.abs);
            } else {
                commands.entity(ent).despawn_recursive();
            }
        } else {
            let pos = map.positions.get(&event.node_id).unwrap().extend(1.);
            commands
                .spawn((
                    NodeIdMarker {
                        node_id: event.node_id.clone(),
                    },
                    SpriteSheetBundle {
                        transform: Transform::default().with_translation(pos),
                        sprite: TextureAtlasSprite {
                            index: event.var.get_sprite_index(),
                            ..Default::default()
                        },
                        texture_atlas: handles.atlas.clone(),
                        ..Default::default()
                    },
                ))
                .with_children(|builder| {
                    builder.spawn((Text2dBundle {
                        text: Text::from_section(format!("{}", event.abs), text_style.clone())
                            .with_alignment(text_alignment),
                        transform: Transform::default().with_translation(Vec3::new(0., 0., 2.)),
                        ..default()
                    },));
                    builder.spawn(SpriteSheetBundle {
                        transform: Transform::default().with_translation(Vec3::new(0., 0., 1.8)),
                        sprite: TextureAtlasSprite {
                            index: 1,
                            ..Default::default()
                        },
                        texture_atlas: handles.atlas.clone(),
                        ..Default::default()
                    });
                });
        }
    }
}

#[derive(Component, Debug, Clone, Default)]
pub struct SpriteInterpolationFx {
    from: Transform,
    to: Transform,
    mid: Option<Transform>,
    timer: Timer,
}

pub fn interpolation_fx(
    mut commands: Commands,
    mut fx_query: Query<(Entity, &mut SpriteInterpolationFx, &mut Transform)>,
    time: Res<Time>,
) {
    for (ent, mut fx, mut tr) in fx_query.iter_mut() {
        fx.timer.tick(time.delta());
        if fx.timer.finished() {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        let t = fx.timer.percent();
        if let Some(mid) = fx.mid {
            if t < 0.5 {
                tr.translation = fx.from.translation.lerp(mid.translation, t * 2.0);
                tr.rotation = fx.from.rotation.lerp(mid.rotation, t * 2.);
            } else {
                tr.translation = mid.translation.lerp(fx.to.translation, (t - 0.5) * 2.0);
                tr.rotation = mid.rotation.lerp(fx.to.rotation, (t - 0.5) * 2.0);
            }
        } else {
            tr.translation = fx.from.translation.lerp(fx.to.translation, t);
            tr.rotation = fx.from.rotation.lerp(fx.to.rotation, t);
        }
    }
}

fn on_modify_resource_fx(
    mut commands: Commands,
    mut events: EventReader<ModifyResourceFx>,
    map: Res<Map>,
    handles: Res<AssetHandles>,
) {
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::BLACK,
    };
    let text_alignment = TextAlignment::Center;

    for event in events.iter() {
        let from = map.positions.get(&event.from).unwrap().extend(1.);
        let to = map.positions.get(&event.to).unwrap().extend(1.);
        commands
            .spawn((
                SpriteSheetBundle {
                    transform: Transform::default().with_translation(from),
                    sprite: TextureAtlasSprite {
                        index: event.var.get_sprite_index(),
                        ..Default::default()
                    },
                    texture_atlas: handles.atlas.clone(),
                    ..Default::default()
                },
                SpriteInterpolationFx {
                    from: Transform::default().with_translation(from),
                    to: Transform::default().with_translation(to),
                    mid: Some(
                        Transform::default()
                            .with_translation((from + to) / 2. + Vec3::new(0., 16., 0.)),
                    ),
                    timer: Timer::new(Duration::from_millis(300), TimerMode::Once),
                },
            ))
            .with_children(|builder| {
                builder.spawn((Text2dBundle {
                    text: Text::from_section(format!("{}", event.diff), text_style.clone())
                        .with_alignment(text_alignment),
                    transform: Transform::default().with_translation(Vec3::new(0., 0., 2.)),
                    ..default()
                },));
                builder.spawn(SpriteSheetBundle {
                    transform: Transform::default().with_translation(Vec3::new(0., 0., 1.8)),
                    sprite: TextureAtlasSprite {
                        index: 1,
                        ..Default::default()
                    },
                    texture_atlas: handles.atlas.clone(),
                    ..Default::default()
                });
            });
    }
}

#[derive(Debug, Clone, Component)]
struct Node {
    id: NodeId,
}
#[derive(Debug, Clone, Component)]
struct Planet {
    id: GroupId,
}
#[derive(Debug, Clone, Component)]
struct Ship {
    orbiting_group: GroupId,
    own_group: GroupId,
    planned_move: Option<GroupId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ResourceVariant {
    Power,
    RocketFuel,
    Food,
    Material,
    FusionFuel,
}

impl ResourceVariant {
    fn get_sprite_index(&self) -> usize {
        match self {
            ResourceVariant::Power => 8,
            ResourceVariant::RocketFuel => 9,
            ResourceVariant::Food => 10,
            ResourceVariant::Material => 11,
            ResourceVariant::FusionFuel => 12,
        }
    }
}

impl ToString for ResourceVariant {
    fn to_string(&self) -> String {
        match self {
            ResourceVariant::Power => "Power",
            ResourceVariant::RocketFuel => "Rocket Fuel",
            ResourceVariant::Food => "Food",
            ResourceVariant::Material => "Material",
            ResourceVariant::FusionFuel => "Fusion Fuel",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Bunch {
    res: HashMap<ResourceVariant, u32>,
}

impl Bunch {
    fn single(var: ResourceVariant, amt: u32) -> Self {
        Self {
            res: HashMap::from([(var, amt)]),
        }
    }

    fn contains(&self, oth: &Bunch) -> bool {
        oth.res.iter().all(|(var, amt)| {
            let Some(cur) = self.res.get(var) else {
                return false;
            };
            cur >= amt
        })
    }
}

impl core::ops::Add for Bunch {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let mut out = self.clone();
        for (var, amt) in rhs.res.iter() {
            if let Some(sum) = out.res.get_mut(var) {
                *sum += amt;
            } else {
                out.res.insert(var.clone(), *amt);
            }
        }
        out
    }
}

impl std::iter::Sum<Self> for Bunch {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::default(), |a, b| a + b)
    }
}

#[derive(Debug, Clone)]
enum ConstructionVariant {
    SolarField,
    HydroponicsFarm,
    AtmosphereHarvester,
    ChemicalPlant,
    BacteriaFarm,
    PlanetFarm,
    AsteroidMine,
    Quarry,
    FusionGenerator,
    RocketGenerator,
    BurnerGenerator,
}

impl ConstructionVariant {
    fn get_sprite_index(&self) -> usize {
        match self {
            Self::SolarField => 16,
            Self::HydroponicsFarm => 17,
            Self::AtmosphereHarvester => 18,
            Self::ChemicalPlant => 19,
            Self::BacteriaFarm => 20,
            Self::PlanetFarm => 21,
            Self::AsteroidMine => 22,
            Self::Quarry => 23,
            Self::FusionGenerator => 24,
            Self::RocketGenerator => 25,
            Self::BurnerGenerator => 26,
        }
    }

    fn get_material_cost(&self) -> u32 {
        match self {
            Self::SolarField => 2,
            Self::HydroponicsFarm => 2,
            Self::AtmosphereHarvester => 20,
            Self::ChemicalPlant => 3,
            Self::BacteriaFarm => 1,
            Self::PlanetFarm => 15,
            Self::AsteroidMine => 4,
            Self::Quarry => 18,
            Self::FusionGenerator => 10,
            Self::RocketGenerator => 1,
            Self::BurnerGenerator => 2,
        }
    }

    fn request_resources(&self) -> Bunch {
        match self {
            Self::SolarField => Bunch::default(),
            Self::HydroponicsFarm => Bunch::single(ResourceVariant::Power, 2),
            Self::AtmosphereHarvester => Bunch::single(ResourceVariant::Power, 4),
            Self::ChemicalPlant => Bunch::single(ResourceVariant::Material, 1),
            Self::BacteriaFarm => Bunch::default(),
            Self::PlanetFarm => Bunch::default(),
            Self::AsteroidMine => Bunch::single(ResourceVariant::RocketFuel, 2),
            Self::Quarry => Bunch::single(ResourceVariant::Power, 5),
            Self::FusionGenerator => Bunch::single(ResourceVariant::FusionFuel, 1),
            Self::RocketGenerator => Bunch::single(ResourceVariant::RocketFuel, 1),
            Self::BurnerGenerator => Bunch::single(ResourceVariant::Food, 1),
        }
    }

    fn produce_resources(&self) -> Bunch {
        match self {
            Self::SolarField => Bunch::single(ResourceVariant::Power, 3),
            Self::HydroponicsFarm => Bunch::single(ResourceVariant::Food, 2),
            Self::AtmosphereHarvester => Bunch::single(ResourceVariant::FusionFuel, 8),
            Self::ChemicalPlant => Bunch::single(ResourceVariant::RocketFuel, 4),
            Self::BacteriaFarm => Bunch::single(ResourceVariant::Food, 3),
            Self::PlanetFarm => Bunch::single(ResourceVariant::Food, 20),
            Self::AsteroidMine => Bunch::single(ResourceVariant::Material, 5),
            Self::Quarry => Bunch::single(ResourceVariant::Material, 60),
            Self::FusionGenerator => Bunch::single(ResourceVariant::Power, 10),
            Self::RocketGenerator => Bunch::single(ResourceVariant::Power, 4),
            Self::BurnerGenerator => Bunch::single(ResourceVariant::Power, 2),
        }
    }

    fn get_cooldown(&self) -> u32 {
        match self {
            Self::AtmosphereHarvester => 3,
            Self::PlanetFarm => 5,
            Self::AsteroidMine => 2,
            Self::Quarry => 3,
            _ => 1,
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [
            Self::SolarField,
            Self::HydroponicsFarm,
            Self::AtmosphereHarvester,
            Self::ChemicalPlant,
            Self::BacteriaFarm,
            Self::PlanetFarm,
            Self::AsteroidMine,
            Self::Quarry,
            Self::FusionGenerator,
            Self::RocketGenerator,
            Self::BurnerGenerator,
        ]
        .iter()
        .cloned()
    }
}

impl ToString for ConstructionVariant {
    fn to_string(&self) -> String {
        match &self {
            Self::SolarField => "Solar Field",
            Self::HydroponicsFarm => "Hydroponics Farm",
            Self::AtmosphereHarvester => "Atmosphere Harvester",
            Self::ChemicalPlant => "Chemical Plant",
            Self::BacteriaFarm => "Bacteria Farm",
            Self::PlanetFarm => "Planet Farm",
            Self::AsteroidMine => "Asteroid Mine",
            Self::Quarry => "Quarry",
            Self::FusionGenerator => "Fusion Generator",
            Self::RocketGenerator => "Rocket Generator",
            Self::BurnerGenerator => "Burner Generator",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, Component)]
struct NodeIdMarker {
    node_id: NodeId,
}

fn setup_scene(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    mut map: ResMut<Map>,
    mut event_construct: EventWriter<BuildConstruction>,
    mut event_produce: EventWriter<ModifyResource>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    commands.spawn(SpriteBundle {
        texture: handles.ship.clone(),
        transform: Transform::default().with_translation(Vec3::new(-80., -360. + 128., 0.0)),
        ..Default::default()
    });

    // planets and nodes
    for (id, pos) in map.group_positions.iter() {
        if id == &GroupId(0) {
            // that's a ship not a planet
            continue;
        }
        commands.spawn((
            SpriteSheetBundle {
                transform: Transform::default().with_translation(pos.extend(0.0)),
                sprite: TextureAtlasSprite {
                    index: 3,
                    ..Default::default()
                },
                texture_atlas: handles.atlas.clone(),
                ..Default::default()
            },
            Planet { id: id.clone() },
        ));
    }
    for (id, pos) in map.positions.iter() {
        commands.spawn((
            SpriteSheetBundle {
                transform: Transform::default().with_translation(pos.extend(0.0)),
                sprite: TextureAtlasSprite {
                    index: 0,
                    ..Default::default()
                },
                texture_atlas: handles.atlas.clone(),
                ..Default::default()
            },
            Node { id: id.clone() },
        ));
    }

    //ship
    let group_pos = map
        .group_positions
        .get(&GroupId(1))
        .unwrap()
        .clone()
        .extend(0.2);
    commands.spawn((
        SpriteSheetBundle {
            transform: Transform::default()
                .with_translation(group_pos + Vec3::new(32., 0., 0.))
                .with_rotation(Quat::from_rotation_z(PI / 2.)),
            sprite: TextureAtlasSprite {
                index: 4,
                ..Default::default()
            },
            texture_atlas: handles.atlas.clone(),
            ..Default::default()
        },
        Ship {
            own_group: GroupId(0),
            orbiting_group: GroupId(1),
            planned_move: None,
        },
    ));

    let occ = NodeOccupant::Construction {
        var: ConstructionVariant::SolarField,
        cooldown: 0,
    };
    map.set_at(&NodeId(0), occ);
    event_construct.send(BuildConstruction {
        node_id: NodeId(0),
        var: ConstructionVariant::SolarField,
    });

    let occ = NodeOccupant::Construction {
        var: ConstructionVariant::HydroponicsFarm,
        cooldown: 0,
    };
    map.set_at(&NodeId(1), occ);
    event_construct.send(BuildConstruction {
        node_id: NodeId(1),
        var: ConstructionVariant::HydroponicsFarm,
    });

    if let Ok(actions) = map.add_resource_in_group(&GroupId(0), &ResourceVariant::FusionFuel, 20) {
        for (to, abs, _diff) in actions {
            event_produce.send(ModifyResource {
                node_id: to.clone(),
                var: ResourceVariant::FusionFuel,
                abs,
            });
        }
    }

    if let Ok(actions) = map.add_resource_in_group(&GroupId(0), &ResourceVariant::Material, 20) {
        for (to, abs, _diff) in actions {
            event_produce.send(ModifyResource {
                node_id: to.clone(),
                var: ResourceVariant::Material,
                abs,
            });
        }
    }

    let (var, node_id) = (ConstructionVariant::Quarry, NodeId(10));
    map.set_at(
        &node_id,
        NodeOccupant::Construction {
            var: var.clone(),
            cooldown: 0,
        },
    );
    event_construct.send(BuildConstruction { node_id, var });
    let (var, node_id) = (ConstructionVariant::SolarField, NodeId(11));
    map.set_at(
        &node_id,
        NodeOccupant::Construction {
            var: var.clone(),
            cooldown: 0,
        },
    );
    event_construct.send(BuildConstruction { node_id, var });
    let (var, node_id) = (ConstructionVariant::SolarField, NodeId(12));
    map.set_at(
        &node_id,
        NodeOccupant::Construction {
            var: var.clone(),
            cooldown: 0,
        },
    );
    event_construct.send(BuildConstruction { node_id, var });

    let (var, node_id) = (ConstructionVariant::Quarry, NodeId(20));
    map.set_at(
        &node_id,
        NodeOccupant::Construction {
            var: var.clone(),
            cooldown: 0,
        },
    );
    event_construct.send(BuildConstruction { node_id, var });
    let (var, node_id) = (ConstructionVariant::SolarField, NodeId(21));
    map.set_at(
        &node_id,
        NodeOccupant::Construction {
            var: var.clone(),
            cooldown: 0,
        },
    );
    event_construct.send(BuildConstruction { node_id, var });

    //if let Ok(events) = map.add_resource_in_group(&GroupId(0), &ResourceVariant::Food, 5) {
    //   event_produce.send_batch(events);
    //}

    next_state.set(AppState::Gameplay);
}

fn ship_orbit(mut query_ship: Query<(&mut Transform, &Ship)>, map: Res<Map>, time: Res<Time>) {
    if let Ok((mut tr, ship)) = query_ship.get_single_mut() {
        let group_pos = map.group_positions.get(&ship.orbiting_group).unwrap();
        tr.rotate_around(
            group_pos.extend(0.),
            Quat::from_rotation_z(0.1 * time.delta_seconds()),
        );
    }
}

fn send_end_turn(keys: Res<Input<KeyCode>>, mut events: EventWriter<EndTurn>) {
    if keys.just_pressed(KeyCode::Delete) {
        events.send(EndTurn);
    }
}

#[derive(Resource, Clone, Debug)]
struct AutoActions {
    actions: Vec<AutoAction>,
    current: Option<AutoAction>,
    timer: Timer,
}

impl Default for AutoActions {
    fn default() -> Self {
        Self {
            actions: vec![],
            current: None,
            timer: Timer::new(Duration::from_millis(300), TimerMode::Repeating),
        }
    }
}

impl AutoActions {
    fn done(&self) -> bool {
        self.actions.is_empty() && self.current.is_none()
    }
}

#[derive(Resource, Clone, Debug)]
enum AutoAction {
    ConsumeResource {
        from: NodeId,
        to: NodeId,
        var: ResourceVariant,
        abs: u32,
        diff: i32,
    },
    ProduceResource {
        from: NodeId,
        to: NodeId,
        var: ResourceVariant,
        abs: u32,
        diff: i32,
    },
    ShipMove {
        from: GroupId,
        to: GroupId,
    },
}

fn turn(
    mut events: EventReader<EndTurn>,
    mut map: ResMut<Map>,
    mut autoactions: ResMut<AutoActions>,
    mut ship_q: Query<&mut Ship>,
) {
    if !autoactions.done() {
        return;
    }
    for _ in events.iter() {
        for (_id, occ) in map.occupation.iter_mut() {
            match occ {
                NodeOccupant::Construction { cooldown, .. } if *cooldown > 0 => {
                    *cooldown -= 1;
                }
                _ => {}
            }
        }
        let mut constructions: Vec<(NodeId, ConstructionVariant)> = map
            .occupation
            .iter()
            .filter_map(|(id, occ)| match occ {
                NodeOccupant::Construction { var, cooldown, .. } if *cooldown <= 0 => {
                    Some((id.clone(), var.clone()))
                }
                _ => None,
            })
            .collect();
        const MAX_TURN_ITERS: usize = 10000;
        for _i in 0..MAX_TURN_ITERS {
            // select a construction that can produce
            let can_produce = constructions.iter().enumerate().find(|(_, (id, var))| {
                let group_id = map.group_from_node(id);
                let available = map.get_group_bunch(&group_id);
                let requested = var.request_resources();
                available.contains(&requested)
            });
            let Some((i, (id, var))) = can_produce else {
                #[cfg(feature = "dbtrace")]
                println!("production starved: {}", constructions.len());
                break;
            };
            if let Some(NodeOccupant::Construction { cooldown, .. }) = map.occupation.get_mut(id) {
                *cooldown = var.get_cooldown();
            }
            #[cfg(feature = "dbtrace")]
            println!("producing with {:?} at {:?}", var, id);
            // delete resources
            // for every requested resource
            let group_id = map.group_from_node(id);
            let requested = var.request_resources();
            for (var, amt) in requested.res.iter() {
                let mut left = amt.clone();
                for _j in 0..MAX_TURN_ITERS {
                    if left == 0 {
                        break;
                    }
                    // delete from the lowest stockpile
                    let lowest_id = map.get_lowest_stockpile(&group_id, var);
                    let NodeOccupant::Stockpile { amt: stock_amt, .. } =
                        map.occupation.get_mut(&lowest_id).expect("no lowest")
                    else {
                        panic!("lowest isn't a stockpile")
                    };
                    let clamped = left.min(*stock_amt);
                    autoactions.actions.push(AutoAction::ConsumeResource {
                        from: lowest_id.clone(),
                        to: id.clone(),
                        var: var.clone(),
                        abs: *stock_amt - clamped,
                        diff: *amt as i32,
                    });
                    *stock_amt -= clamped;
                    if *stock_amt == 0 {
                        map.occupation.remove(&lowest_id);
                    }
                    left -= clamped;
                }
            }
            // then add the produced
            let produced = var.produce_resources();
            for (var, amt) in produced.res.iter() {
                if let Ok(actions) = map.add_resource_in_group(&group_id, var, *amt) {
                    for (to, abs, diff) in actions {
                        autoactions.actions.push(AutoAction::ProduceResource {
                            from: id.clone(),
                            to,
                            var: var.clone(),
                            abs,
                            diff,
                        });
                    }
                }
            }
            constructions.remove(i);
        }
        // todo:decay

        if let Ok(mut ship) = ship_q.get_single_mut() {
            if let Some(plan) = ship.planned_move.clone() {
                let fusion = *map
                    .get_group_bunch(&GroupId(0))
                    .res
                    .get(&ResourceVariant::FusionFuel)
                    .unwrap_or(&0);
                if fusion > 0 {
                    let lowest_id =
                        map.get_lowest_stockpile(&GroupId(0), &ResourceVariant::FusionFuel);
                    let Some(NodeOccupant::Stockpile { amt, .. }) =
                        map.occupation.get_mut(&lowest_id)
                    else {
                        return;
                    };
                    *amt -= 1;
                    autoactions.actions.push(AutoAction::ConsumeResource {
                        from: lowest_id.clone(),
                        to: lowest_id.clone(),
                        var: ResourceVariant::FusionFuel,
                        abs: *amt,
                        diff: -1,
                    });
                    autoactions.actions.push(AutoAction::ShipMove {
                        from: ship.orbiting_group.clone(),
                        to: plan.clone(),
                    });
                    // AAAAAAAAAAAAAAH!
                    unsafe {
                        // modify the graph to set as adjacent the ship's group
                        map.edges
                            .retain(|edge| edge.0 != ship.own_group && edge.1 != ship.own_group);
                        map.edges.push((ship.own_group.clone(), plan.clone()))
                    }
                }
            }

            ship.planned_move = None;
        }

        // hack to just start the anim
        autoactions.timer.tick(Duration::from_secs(1));
    }
}

fn play_autoactions(
    mut autoactions: ResMut<AutoActions>,
    mut event_produce: EventWriter<ModifyResource>,
    mut fx: EventWriter<ModifyResourceFx>,
    mut ship_q: Query<(&mut Ship, &mut Visibility, &mut Transform)>,
    map: Res<Map>,
    time: Res<Time>,
    mut commands: Commands,
    handles: Res<AssetHandles>,
) {
    autoactions.timer.tick(time.delta());
    if autoactions.timer.finished() {
        if let Some(act) = &autoactions.current {
            // sync state at end of actions
            match &act {
                AutoAction::ConsumeResource {
                    from,
                    to: _,
                    var,
                    abs,
                    diff: _,
                } => {
                    event_produce.send(ModifyResource {
                        node_id: from.clone(),
                        var: var.clone(),
                        abs: *abs,
                    });
                }
                AutoAction::ProduceResource {
                    from: _,
                    to,
                    var,
                    abs,
                    diff: _,
                } => {
                    event_produce.send(ModifyResource {
                        node_id: to.clone(),
                        var: var.clone(),
                        abs: *abs,
                    });
                }
                AutoAction::ShipMove { from, to } => {
                    if let Ok((mut ship, mut vis, _)) = ship_q.get_single_mut() {
                        ship.orbiting_group = to.clone();
                        *vis = Visibility::Visible;
                    }
                }
            };
        }
        if autoactions.actions.is_empty() {
            autoactions.current = None;
            return;
        }
        let act = autoactions.actions.remove(0);
        // spawn fx at start of action
        match &act {
            AutoAction::ConsumeResource {
                from,
                to,
                var,
                abs: _,
                diff,
            } => {
                fx.send(ModifyResourceFx {
                    from: from.clone(),
                    to: to.clone(),
                    var: var.clone(),
                    diff: *diff,
                });
            }
            AutoAction::ProduceResource {
                from,
                to,
                var,
                abs: _,
                diff,
            } => {
                fx.send(ModifyResourceFx {
                    from: from.clone(),
                    to: to.clone(),
                    var: var.clone(),
                    diff: *diff,
                });
            }
            AutoAction::ShipMove { from, to } => {
                if let Ok((_ship, mut vis, mut tr)) = ship_q.get_single_mut() {
                    let from = tr.clone();
                    tr.translation =
                        map.group_positions.get(to).unwrap().extend(0.2) + Vec3::new(32., 0., 0.);
                    tr.rotation = Quat::from_rotation_z(PI / 2.);
                    let to = tr.clone();
                    *vis = Visibility::Hidden;
                    commands.spawn((
                        SpriteSheetBundle {
                            transform: from,
                            sprite: TextureAtlasSprite {
                                index: 4,
                                ..Default::default()
                            },
                            texture_atlas: handles.atlas.clone(),
                            ..Default::default()
                        },
                        SpriteInterpolationFx {
                            from: from,
                            to: to,
                            mid: None,
                            timer: Timer::new(Duration::from_millis(300), TimerMode::Once),
                        },
                    ));
                }
            }
        };
        autoactions.current = Some(act);
    }
}

#[derive(Clone, Debug, Component)]
struct Highlight;
#[derive(Clone, Debug, Component)]
struct Selected;
#[derive(Clone, Debug, Component)]
struct SelectedMove;

fn highlight(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    query_highlight: Query<(Entity, &Highlight)>,
    query_nodes: Query<(Entity, &Node)>,
    query_planets: Query<(Entity, &Planet)>,
    query_tr: Query<&Transform>,
    query_windows: Query<&Window, With<PrimaryWindow>>,
    query_camera: Query<(&Camera, &GlobalTransform)>,
    mut event_ui: EventWriter<UiEvent>,
    mouse_button_input: Res<Input<MouseButton>>,
    query_moving_to: Query<&MovingTo>,
    query_ship_moving_to: Query<&ShipMovingTo>,
    mut map: ResMut<Map>,
    mut ship: Query<&mut Ship>,
    mut autoactions: ResMut<AutoActions>,
) {
    for (e, _) in query_highlight.iter() {
        commands.entity(e).despawn();
    }

    let clicked = mouse_button_input.just_pressed(MouseButton::Left);

    let Some(mouse_viewport) = query_windows.single().cursor_position() else {
        return;
    };

    let (camera, camera_transform) = query_camera.single();
    let mouse = camera
        .viewport_to_world_2d(camera_transform, mouse_viewport)
        .unwrap_or(Vec2::ZERO);

    let mut found_node = None;
    for (ent, node) in query_nodes.iter() {
        let tr = query_tr.get(ent).unwrap();
        let rect = Rect::from_center_size(
            Vec2::new(tr.translation.x, tr.translation.y),
            Vec2::new(64., 64.),
        );
        if rect.contains(mouse) {
            found_node = Some((tr, node));
        }
    }

    if let Some((tr, node)) = found_node {
        commands.spawn((
            SpriteSheetBundle {
                transform: Transform::default()
                    .with_translation(tr.translation + Vec3::new(0., 0., 3.)),
                sprite: TextureAtlasSprite {
                    color: Color::WHITE.with_a(0.3),
                    index: 2,
                    ..Default::default()
                },
                texture_atlas: handles.atlas.clone(),
                ..Default::default()
            },
            Highlight,
        ));
        if clicked {
            if let Ok(MovingTo(from_id, nears)) = query_moving_to.get_single() {
                if nears.contains(&node.id) && map.occupation.get(&node.id).is_none() {
                    let Some(NodeOccupant::Stockpile { var, amt }) = map.occupation.remove(from_id)
                    else {
                        return;
                    };
                    map.occupation.insert(
                        node.id.clone(),
                        NodeOccupant::Stockpile {
                            var: var.clone(),
                            amt,
                        },
                    );
                    autoactions.actions.push(AutoAction::ConsumeResource {
                        from: from_id.clone(),
                        to: node.id.clone(),
                        var: var.clone(),
                        abs: 0,
                        diff: amt as i32,
                    });
                    autoactions.actions.push(AutoAction::ProduceResource {
                        from: node.id.clone(),
                        to: node.id.clone(),
                        var: var.clone(),
                        abs: amt,
                        diff: amt as i32,
                    });
                    autoactions.timer.tick(Duration::from_secs(1));
                    event_ui.send(UiEvent::Close);
                    return;
                }
            }
            event_ui.send(UiEvent::SelectNodeForConstruction(node.id.clone()));
        }
    }

    let mut found_planet = None;
    for (ent, node) in query_planets.iter() {
        let tr = query_tr.get(ent).unwrap();
        let rect = Rect::from_center_size(
            Vec2::new(tr.translation.x, tr.translation.y),
            Vec2::new(64., 64.),
        );
        if rect.contains(mouse) {
            found_planet = Some((tr, node));
        }
    }

    if let Some((tr, planet)) = found_planet {
        commands.spawn((
            SpriteSheetBundle {
                transform: Transform::default()
                    .with_translation(tr.translation + Vec3::new(0., 0., 3.)),
                sprite: TextureAtlasSprite {
                    color: Color::WHITE.with_a(0.3),
                    index: 5,
                    ..Default::default()
                },
                texture_atlas: handles.atlas.clone(),
                ..Default::default()
            },
            Highlight,
        ));
        if clicked {
            if let Ok(ShipMovingTo(from_id, nears)) = query_ship_moving_to.get_single() {
                let fusion = *map
                    .get_group_bunch(&GroupId(0))
                    .res
                    .get(&ResourceVariant::FusionFuel)
                    .unwrap_or(&0);
                if fusion > 0 {
                    if let Ok(mut ship) = ship.get_single_mut() {
                        ship.planned_move = Some(planet.id.clone());
                    }
                }
                event_ui.send(UiEvent::Close);
            } else {
                event_ui.send(UiEvent::SelectPlanet(planet.id.clone()));
            }
        }
    }
}

#[derive(Component)]
enum UiButton {
    ConstructMenu(NodeId),
    DestroyMenu(NodeId),
    MoveMenu(NodeId),
    Construct(NodeId, ConstructionVariant),
}

#[derive(Component)]
struct UiNodeSelectedPlanet;

#[derive(Component)]
struct ShipMovingTo(GroupId, Vec<GroupId>);

fn ui_on_node_selected_planet(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    mut event_ui: EventReader<UiEvent>,
    query_ui_sel: Query<(Entity, &UiNodeSelectedMove)>,
    query_selected: Query<(Entity, &SelectedMove)>,
    query_moving_to: Query<(Entity, &ShipMovingTo)>,
    map: Res<Map>,
    ship: Query<&Ship>,
) {
    if event_ui.is_empty() {
        return;
    }

    for (e, _) in query_moving_to.iter() {
        commands.entity(e).despawn_recursive();
    }
    for (e, _) in query_ui_sel.iter() {
        commands.entity(e).despawn_recursive();
    }
    for (e, _) in query_selected.iter() {
        commands.entity(e).despawn_recursive();
    }

    let event = event_ui
        .iter()
        .find(|e| matches!(e, UiEvent::SelectPlanet(_)));
    let Some(UiEvent::SelectPlanet(group_id)) = event else {
        return;
    };

    if let Ok(ship) = ship.get_single() {
        if ship.orbiting_group != *group_id {
            return;
        }
    }

    let big_text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::WHITE,
    };
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 20.0,
        color: Color::WHITE,
    };

    let star = map.star(&group_id);

    let mut nears = vec![];
    for neighbor_group_id in star.iter() {
        if neighbor_group_id == &GroupId(0) {
            // that's a ship not a planet
            continue;
        }
        nears.push(group_id.clone());
        let pos = map.group_positions.get(neighbor_group_id).unwrap();
        commands.spawn((
            SpriteSheetBundle {
                transform: Transform::default().with_translation(pos.extend(4.)),
                sprite: TextureAtlasSprite {
                    index: 5,
                    ..Default::default()
                },
                texture_atlas: handles.atlas.clone(),
                ..Default::default()
            },
            SelectedMove,
        ));
    }

    commands.spawn(ShipMovingTo(group_id.clone(), nears));

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(0.),
                    right: Val::Percent(0.),
                    width: Val::Percent(25.),
                    height: Val::Percent(100.),
                    border: UiRect::all(Val::Px(5.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                background_color: Color::rgb(0.1, 0.1, 0.1).into(),
                border_color: Color::WHITE.into(),
                ..default()
            },
            UiNodeSelectedMove,
        ))
        .with_children(|root| {
            let fusion = *map
                .get_group_bunch(&GroupId(0))
                .res
                .get(&ResourceVariant::FusionFuel)
                .unwrap_or(&0);
            root.spawn(
                TextBundle::from_section(
                    format!(
                        "Select a destination.\nTraveling will use 1 Fusion Fuel.\nIn the ship there is {} Fusion Fuel.",
                        fusion,
                    ),
                    text_style.clone(),
                )
                .with_style(Style {
                    position_type: PositionType::Relative,
                    margin: UiRect::vertical(Val::Px(10.)),
                    ..default()
                }),
            );
        });
}

#[derive(Component)]
struct UiNodeSelectedMove;

#[derive(Component)]
struct MovingTo(NodeId, Vec<NodeId>);

fn ui_on_node_selected_move(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    mut event_ui: EventReader<UiEvent>,
    query_ui_sel: Query<(Entity, &UiNodeSelectedMove)>,
    query_selected: Query<(Entity, &SelectedMove)>,
    query_moving_to: Query<(Entity, &MovingTo)>,
    map: Res<Map>,
) {
    if event_ui.is_empty() {
        return;
    }

    for (e, _) in query_moving_to.iter() {
        commands.entity(e).despawn_recursive();
    }
    for (e, _) in query_ui_sel.iter() {
        commands.entity(e).despawn_recursive();
    }
    for (e, _) in query_selected.iter() {
        commands.entity(e).despawn_recursive();
    }

    let event = event_ui
        .iter()
        .find(|e| matches!(e, UiEvent::SelectNodeForMove(_)));
    let Some(UiEvent::SelectNodeForMove(id)) = event else {
        return;
    };

    let big_text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::WHITE,
    };
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 20.0,
        color: Color::WHITE,
    };

    let group_id = map.group_from_node(id);
    let star = map.star(&group_id);

    let mut nears = vec![];
    for neighbor_group_id in star.iter() {
        for node_id in map.groups.get(neighbor_group_id).unwrap().iter() {
            nears.push(node_id.clone());
            let pos = map.positions.get(node_id).unwrap();
            // if planet, red
            commands.spawn((
                SpriteSheetBundle {
                    transform: Transform::default().with_translation(pos.extend(4.)),
                    sprite: TextureAtlasSprite {
                        index: 2,
                        ..Default::default()
                    },
                    texture_atlas: handles.atlas.clone(),
                    ..Default::default()
                },
                SelectedMove,
            ));
        }
    }

    commands.spawn(MovingTo(id.clone(), nears));

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(0.),
                    right: Val::Percent(0.),
                    width: Val::Percent(25.),
                    height: Val::Percent(100.),
                    border: UiRect::all(Val::Px(5.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                background_color: Color::rgb(0.1, 0.1, 0.1).into(),
                border_color: Color::WHITE.into(),
                ..default()
            },
            UiNodeSelectedMove,
        ))
        .with_children(|root| {
            root.spawn(
                TextBundle::from_section("Select a destination", big_text_style.clone())
                    .with_style(Style {
                        position_type: PositionType::Relative,
                        top: Val::Percent(0.),
                        right: Val::Percent(0.),
                        margin: UiRect::bottom(Val::Px(10.)),
                        ..default()
                    }),
            );
        });
}

#[derive(Component)]
struct UiNodeSelectedConstr;

fn ui_on_node_selected_constr(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    mut event_ui: EventReader<UiEvent>,
    query_ui_sel: Query<(Entity, &UiNodeSelectedConstr)>,
    query_selected: Query<(Entity, &Selected)>,
    map: Res<Map>,
) {
    if event_ui.is_empty() {
        return;
    }

    for (e, _) in query_ui_sel.iter() {
        commands.entity(e).despawn_recursive();
    }
    for (e, _) in query_selected.iter() {
        commands.entity(e).despawn_recursive();
    }

    let event = event_ui
        .iter()
        .find(|e| matches!(e, UiEvent::SelectNodeForConstruction(_)));
    let Some(UiEvent::SelectNodeForConstruction(id)) = event else {
        return;
    };

    let big_text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::WHITE,
    };
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 20.0,
        color: Color::WHITE,
    };
    let small_text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 10.0,
        color: Color::WHITE,
    };

    let pos = map.positions.get(id).unwrap();
    commands.spawn((
        SpriteSheetBundle {
            transform: Transform::default().with_translation(pos.extend(4.)),
            sprite: TextureAtlasSprite {
                index: 2,
                ..Default::default()
            },
            texture_atlas: handles.atlas.clone(),
            ..Default::default()
        },
        Selected,
    ));

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(0.),
                    right: Val::Percent(0.),
                    width: Val::Percent(25.),
                    height: Val::Percent(100.),
                    border: UiRect::all(Val::Px(5.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                background_color: Color::rgb(0.1, 0.1, 0.1).into(),
                border_color: Color::WHITE.into(),
                ..default()
            },
            UiNodeSelectedConstr,
        ))
        .with_children(|root| {
            root.spawn(
                TextBundle::from_section("Available Actions", big_text_style.clone()).with_style(
                    Style {
                        position_type: PositionType::Relative,
                        top: Val::Percent(0.),
                        right: Val::Percent(0.),
                        margin: UiRect::bottom(Val::Px(10.)),
                        ..default()
                    },
                ),
            );
            let group_id = map.group_from_node(id);
            let is_ship_present =
                group_id == GroupId(0) || map.star(&GroupId(0)).contains(&group_id);
            if !is_ship_present {
                root.spawn(
                    TextBundle::from_section(
                        "Your ship is too far away from this location.\nYou can move the ship\
                        closer if you have 1 Fusion Fuel.",
                        text_style.clone(),
                    )
                    .with_style(Style {
                        margin: UiRect::bottom(Val::Px(10.)),
                        ..default()
                    }),
                );
                return;
            }
            let occ = map.occupation.get(id);
            if occ.is_none() {
                root.spawn((
                    ButtonBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            border: UiRect::all(Val::Px(3.0)),
                            margin: UiRect::all(Val::Px(2.)),
                            ..Default::default()
                        },
                        background_color: Color::rgb(0.14, 0.14, 0.14).into(),
                        border_color: Color::rgb(0.2, 0.2, 0.2).into(),
                        ..Default::default()
                    },
                    UiButton::ConstructMenu(id.clone()),
                ))
                .with_children(|button| {
                    button.spawn(
                        TextBundle::from_section("Construct", big_text_style.clone()).with_style(
                            Style {
                                position_type: PositionType::Relative,
                                ..default()
                            },
                        ),
                    );
                });
            } else if matches!(occ, Some(NodeOccupant::Construction { .. })) {
                root.spawn((
                    ButtonBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            border: UiRect::all(Val::Px(3.0)),
                            margin: UiRect::all(Val::Px(2.)),
                            ..Default::default()
                        },
                        background_color: Color::rgb(0.14, 0.14, 0.14).into(),
                        border_color: Color::rgb(0.2, 0.2, 0.2).into(),
                        ..Default::default()
                    },
                    UiButton::DestroyMenu(id.clone()),
                ))
                .with_children(|button| {
                    button.spawn(
                        TextBundle::from_section("Demolish", big_text_style.clone()).with_style(
                            Style {
                                position_type: PositionType::Relative,
                                ..default()
                            },
                        ),
                    );
                });
            } else if let Some(NodeOccupant::Stockpile { var, amt }) = occ {
                root.spawn((
                    ButtonBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            border: UiRect::all(Val::Px(3.0)),
                            margin: UiRect::all(Val::Px(2.)),
                            ..Default::default()
                        },
                        background_color: Color::rgb(0.14, 0.14, 0.14).into(),
                        border_color: Color::rgb(0.2, 0.2, 0.2).into(),
                        ..Default::default()
                    },
                    UiButton::MoveMenu(id.clone()),
                ))
                .with_children(|button| {
                    button.spawn(
                        TextBundle::from_section("Move Resources", big_text_style.clone())
                            .with_style(Style {
                                position_type: PositionType::Relative,
                                ..default()
                            }),
                    );
                });
            }
        });
}

fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &UiButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut event_ui: EventWriter<UiEvent>,
    mut event_construct: EventWriter<BuildConstruction>,
    mut event_destruct: EventWriter<DestroyConstruction>,
    mut map: ResMut<Map>,
    mut autoactions: ResMut<AutoActions>,
) {
    for (interaction, mut color, ui_button) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::RED.into();
                match ui_button {
                    UiButton::ConstructMenu(id) => {
                        event_ui.send(UiEvent::ConstructOnNode(id.clone()));
                    }
                    UiButton::Construct(node_id, var) => {
                        let group_id = map.group_from_node(node_id);
                        let cash = *map
                            .get_group_bunch(&group_id)
                            .res
                            .get(&ResourceVariant::Material)
                            .clone()
                            .unwrap_or(&0);
                        let can_buy = cash >= var.get_material_cost();
                        if map.occupation.get(node_id).is_none() && can_buy {
                            let occ = NodeOccupant::Construction {
                                var: var.clone(),
                                cooldown: 0,
                            };
                            map.set_at(node_id, occ);
                            event_construct.send(BuildConstruction {
                                node_id: node_id.clone(),
                                var: var.clone(),
                            });
                            event_ui.send(UiEvent::SelectNodeForConstruction(node_id.clone()));
                            let group_id = map.group_from_node(node_id);
                            let requested =
                                Bunch::single(ResourceVariant::Material, var.get_material_cost());
                            for (var, amt) in requested.res.iter() {
                                let mut left = amt.clone();
                                for _j in 0..16 {
                                    if left == 0 {
                                        break;
                                    }
                                    // delete from the lowest stockpile
                                    let lowest_id = map.get_lowest_stockpile(&group_id, var);
                                    let NodeOccupant::Stockpile { amt: stock_amt, .. } =
                                        map.occupation.get_mut(&lowest_id).expect("no lowest")
                                    else {
                                        panic!("lowest isn't a stockpile")
                                    };
                                    let clamped = left.min(*stock_amt);
                                    autoactions.actions.push(AutoAction::ConsumeResource {
                                        from: lowest_id.clone(),
                                        to: node_id.clone(),
                                        var: var.clone(),
                                        abs: *stock_amt - clamped,
                                        diff: *amt as i32,
                                    });
                                    *stock_amt -= clamped;
                                    if *stock_amt == 0 {
                                        map.occupation.remove(&lowest_id);
                                    }
                                    left -= clamped;
                                }
                            }
                            autoactions.timer.tick(Duration::from_secs(1));
                        }
                    }
                    UiButton::DestroyMenu(node_id) => {
                        map.occupation.remove(node_id);
                        event_destruct.send(DestroyConstruction {
                            node_id: node_id.clone(),
                        });
                        event_ui.send(UiEvent::Close);
                    }
                    UiButton::MoveMenu(node_id) => {
                        event_ui.send(UiEvent::SelectNodeForMove(node_id.clone()));
                    }
                }
            }
            Interaction::Hovered => {
                *color = Color::BLACK.with_a(0.3).into();
            }
            Interaction::None => {
                *color = Color::BLACK.into();
            }
        }
    }
}

#[derive(Component)]
struct UiConstruct;

fn ui_on_construction(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    query_ui_cons: Query<(Entity, &UiConstruct)>,
    mut event_ui: EventReader<UiEvent>,
    map: Res<Map>,
) {
    if event_ui.is_empty() {
        return;
    }

    for (e, _) in query_ui_cons.iter() {
        commands.entity(e).despawn_recursive();
    }

    let event = event_ui
        .iter()
        .find(|e| matches!(e, UiEvent::ConstructOnNode(_)));
    let Some(UiEvent::ConstructOnNode(id)) = event else {
        return;
    };

    let pos = map.positions.get(id).unwrap();
    commands.spawn((
        SpriteSheetBundle {
            transform: Transform::default().with_translation(pos.extend(4.)),
            sprite: TextureAtlasSprite {
                index: 2,
                ..Default::default()
            },
            texture_atlas: handles.atlas.clone(),
            ..Default::default()
        },
        Selected,
    ));

    let big_text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::WHITE,
    };
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 20.0,
        color: Color::WHITE,
    };
    let small_text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 10.0,
        color: Color::WHITE,
    };
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(0.),
                    right: Val::Percent(0.),
                    width: Val::Percent(25.),
                    height: Val::Percent(100.),
                    border: UiRect::all(Val::Px(5.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                background_color: Color::rgb(0.1, 0.1, 0.1).into(),
                border_color: Color::WHITE.into(),
                ..default()
            },
            UiConstruct,
        ))
        .with_children(|root| {
            root.spawn(
                TextBundle::from_section("Construct", big_text_style.clone()).with_style(Style {
                    position_type: PositionType::Relative,
                    top: Val::Percent(0.),
                    right: Val::Percent(0.),
                    margin: UiRect::bottom(Val::Px(10.)),
                    ..default()
                }),
            );
            for constr in ConstructionVariant::iter() {
                let group_id = map.group_from_node(id);
                let cash = *map
                    .get_group_bunch(&group_id)
                    .res
                    .get(&ResourceVariant::Material)
                    .clone()
                    .unwrap_or(&0);
                let can_buy = cash >= constr.get_material_cost();
                root.spawn((
                    ButtonBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            border: UiRect::all(Val::Px(3.0)),
                            margin: UiRect::all(Val::Px(2.)),
                            ..Default::default()
                        },
                        background_color: Color::rgb(0.14, 0.14, 0.14).into(),
                        border_color: Color::rgb(0.2, 0.2, 0.2).into(),
                        ..Default::default()
                    },
                    UiButton::Construct(id.clone(), constr.clone()),
                ))
                .with_children(|constr_node| {
                    constr_node.spawn(AtlasImageBundle {
                        style: Style {
                            width: Val::Px(48.),
                            height: Val::Px(48.),
                            ..Default::default()
                        },
                        texture_atlas: handles.atlas.clone(),
                        texture_atlas_image: UiTextureAtlasImage {
                            index: constr.get_sprite_index(),
                            ..Default::default()
                        },
                        ..Default::default()
                    });
                    constr_node
                        .spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .with_children(|details| {
                            details.spawn(
                                TextBundle::from_section(constr.to_string(), text_style.clone())
                                    .with_style(Style {
                                        position_type: PositionType::Relative,
                                        ..default()
                                    }),
                            );
                            details.spawn(
                                TextBundle::from_section(
                                    format!(
                                        "cost: {} {}, you have {} {} in this sector",
                                        constr.get_material_cost(),
                                        ResourceVariant::Material.to_string(),
                                        cash,
                                        ResourceVariant::Material.to_string()
                                    ),
                                    if can_buy {
                                        small_text_style.clone()
                                    } else {
                                        TextStyle {
                                            font: handles.font.clone(),
                                            font_size: 10.0,
                                            color: Color::RED,
                                        }
                                    },
                                )
                                .with_style(Style {
                                    position_type: PositionType::Relative,
                                    ..default()
                                }),
                            );
                            // assuming one bunch
                            let prod = constr.produce_resources();
                            let cons = constr.request_resources();
                            let (mut pvar, mut pamt) = (ResourceVariant::Power, 0);
                            for (var, amt) in prod.res.iter() {
                                pvar = var.clone();
                                pamt = amt.clone();
                            }
                            let (mut cvar, mut camt) = (ResourceVariant::Power, 0);
                            for (var, amt) in cons.res.iter() {
                                cvar = var.clone();
                                camt = amt.clone();
                            }
                            let cooldown = constr.get_cooldown();
                            details.spawn(
                                TextBundle::from_section(
                                    format!(
                                        "generates: {} {} using {} {} every {} turns",
                                        pamt,
                                        pvar.to_string(),
                                        camt,
                                        cvar.to_string(),
                                        cooldown
                                    ),
                                    small_text_style.clone(),
                                )
                                .with_style(Style {
                                    position_type: PositionType::Relative,
                                    ..default()
                                }),
                            );
                        });
                });
            }
        });
}
