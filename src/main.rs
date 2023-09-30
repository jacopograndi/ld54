use std::time::Duration;

use bevy::{app::AppExit, asset::LoadState, prelude::*, utils::HashMap};

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
        .add_systems(OnEnter(AppState::Setup), (setup_ui, setup_scene).chain())
        .add_systems(Update, escape_exit)
        .add_systems(
            Update,
            (turn, on_build_construction, on_modify_resource)
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
            (interpolation_fx, on_modify_resource_fx).run_if(in_state(AppState::Gameplay)),
        )
        .insert_resource(AssetHandles::default())
        .insert_resource(Map::new())
        .insert_resource(AutoActions::default())
        .add_event::<EndTurn>()
        .add_event::<BuildConstruction>()
        .add_event::<DestroyConstruction>()
        .add_event::<ModifyResource>()
        .add_event::<ModifyResourceFx>()
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
}

fn startup(
    mut commands: Commands,
    mut handles: ResMut<AssetHandles>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2dBundle::default());
    handles.sheet = asset_server.load("sheet.png");
    handles.font = asset_server.load("FFFFORWA.TTF");
}

fn check_loading(
    handles: Res<AssetHandles>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    match asset_server.get_load_state(handles.sheet.clone()) {
        LoadState::Loaded => next_state.set(AppState::Setup),
        LoadState::Failed => panic!("load base_sheet.png failed"),
        _ => (),
    }
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
    edges: Vec<(NodeId, NodeId)>,
    positions: HashMap<NodeId, Vec2>,
    occupation: HashMap<NodeId, NodeOccupant>,
}

#[derive(Debug, Clone, Deref, DerefMut, PartialEq, Eq, Hash)]
struct NodeId(usize);
#[derive(Debug, Clone, Deref, DerefMut, PartialEq, Eq, Hash)]
struct GroupId(usize);

#[derive(Debug, Clone)]
enum NodeOccupant {
    Construction { var: ConstructionVariant },
    Stockpile { var: ResourceVariant, amt: u32 },
}

const MAX_STOCKPILE: u32 = 100;

impl Map {
    fn new() -> Self {
        let nodes: Vec<NodeId> = (0..5).map(|i| NodeId(i)).collect();
        let mut map = Self {
            nodes: nodes.clone(),
            groups: HashMap::from([(GroupId(0), nodes.clone())]),
            edges: vec![],
            positions: HashMap::from([
                (NodeId(0), Vec2::new(0., 0.)),
                (NodeId(1), Vec2::new(64., 0.)),
                (NodeId(2), Vec2::new(128., 0.)),
                (NodeId(3), Vec2::new(-64., 0.)),
                (NodeId(4), Vec2::new(-128., 0.)),
            ]),
            occupation: HashMap::default(),
        };
        map
    }

    fn group_from_node(&mut self, id: &NodeId) -> GroupId {
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
                    actions.push((empty_id.clone(), amt, amt as i32));
                    self.set_at(
                        empty_id,
                        NodeOccupant::Stockpile {
                            var: v.clone(),
                            amt,
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
    mut stock_q: Query<(Entity, &NodeIdMarker, &mut Text)>,
    map: Res<Map>,
    handles: Res<AssetHandles>,
) {
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 30.0,
        color: Color::WHITE,
    };
    let text_alignment = TextAlignment::Center;

    for event in events.iter() {
        if let Some((ent, _, mut text)) = stock_q
            .iter_mut()
            .find(|(_, marker, _)| marker.node_id == event.node_id)
        {
            if let NodeOccupant::Stockpile { amt: stock_amt, .. } =
                map.occupation.get(&event.node_id).unwrap()
            {
                if *stock_amt > 0 {
                    text.sections[0].value = format!("{}", event.abs);
                } else {
                    commands.entity(ent).despawn_recursive();
                }
            } else {
                panic!();
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
                    builder.spawn((
                        Text2dBundle {
                            text: Text::from_section(format!("{}", event.abs), text_style.clone())
                                .with_alignment(text_alignment),
                            transform: Transform::default().with_translation(Vec3::new(0., 0., 2.)),
                            ..default()
                        },
                        NodeIdMarker {
                            node_id: event.node_id.clone(),
                        },
                    ));
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
            } else {
                tr.translation = mid.translation.lerp(fx.to.translation, (t - 0.5) * 2.0);
            }
        } else {
            tr.translation = fx.from.translation.lerp(fx.to.translation, t as f32);
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
        color: Color::WHITE,
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
            });
    }
}

// components
#[derive(Debug, Clone, Component)]
struct Node {
    id: NodeId,
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
}

impl ConstructionVariant {
    fn get_sprite_index(&self) -> usize {
        match self {
            ConstructionVariant::SolarField => 16,
            ConstructionVariant::HydroponicsFarm => 17,
        }
    }

    fn request_resources(&self) -> Bunch {
        match self {
            Self::SolarField => Bunch::default(),
            Self::HydroponicsFarm => Bunch::single(ResourceVariant::Power, 2),
        }
    }

    fn produce_resources(&self) -> Bunch {
        match self {
            Self::SolarField => Bunch::single(ResourceVariant::Power, 3),
            Self::HydroponicsFarm => Bunch::single(ResourceVariant::Food, 2),
        }
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

    let occ = NodeOccupant::Construction {
        var: ConstructionVariant::SolarField,
    };
    map.set_at(&NodeId(0), occ);
    event_construct.send(BuildConstruction {
        node_id: NodeId(0),
        var: ConstructionVariant::SolarField,
    });

    let occ = NodeOccupant::Construction {
        var: ConstructionVariant::HydroponicsFarm,
    };
    map.set_at(&NodeId(1), occ);
    event_construct.send(BuildConstruction {
        node_id: NodeId(1),
        var: ConstructionVariant::HydroponicsFarm,
    });

    //if let Ok(events) = map.add_resource_in_group(&GroupId(0), &ResourceVariant::Food, 5) {
    //   event_produce.send_batch(events);
    //}

    next_state.set(AppState::Gameplay);
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
}

fn turn(
    mut events: EventReader<EndTurn>,
    mut map: ResMut<Map>,
    mut autoactions: ResMut<AutoActions>,
) {
    if !autoactions.done() {
        return;
    }
    for _ in events.iter() {
        let mut constructions: Vec<(NodeId, ConstructionVariant)> = map
            .occupation
            .iter()
            .filter_map(|(id, occ)| match occ {
                NodeOccupant::Construction { var } => Some((id.clone(), var.clone())),
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

        // hack to just start the anim
        autoactions.timer.tick(Duration::from_secs(1));
    }
}

fn play_autoactions(
    mut autoactions: ResMut<AutoActions>,
    mut event_produce: EventWriter<ModifyResource>,
    mut fx: EventWriter<ModifyResourceFx>,
    time: Res<Time>,
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
                abs,
                diff,
            } => {
                fx.send(ModifyResourceFx {
                    from: from.clone(),
                    to: to.clone(),
                    var: var.clone(),
                    diff: *diff,
                });
            }
        };
        autoactions.current = Some(act);
    }
}

fn setup_ui(mut cmd: Commands, handles: Res<AssetHandles>) {
    let text_style = TextStyle {
        font: handles.font.clone(),
        font_size: 54.0,
        color: Color::WHITE,
    };
    cmd.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            ..default()
        },
        ..default()
    })
    .with_children(|root| {
        root.spawn(
            TextBundle::from_section("Resolution\nAAAAAAAAAAA", text_style.clone()).with_style(
                Style {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(0.),
                    left: Val::Percent(0.),
                    ..default()
                },
            ),
        );
        root.spawn(
            TextBundle::from_section("Resolution\nBBBBBBBBBB", text_style.clone()).with_style(
                Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(0.),
                    right: Val::Percent(0.),
                    ..default()
                },
            ),
        );
    });
}
