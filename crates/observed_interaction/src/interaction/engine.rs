//! The **interaction engine**: the pure tick that resolves player intents against
//! the interaction world — activate/operate, exclusive/shared holds with quorum and
//! interruption, pick up / drop / socket / recover equipment, and climb — plus the
//! contextual prompt. Deterministic (players resolved in `PlayerId` order).

use glam::Vec2;
use observed_core::{PlayerId, PlayerIntent};

use super::model::{
    EquipmentId, InteractionEvent, InteractionId, InteractionKind, InteractionPolicy,
    InteractionWorld, ItemLocation, SocketId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionPrompt {
    pub target: Option<InteractionId>,
    pub text: String,
}

pub fn tick_interactions(
    world: &mut InteractionWorld,
    intents: &[(PlayerId, PlayerIntent)],
    dt: f32,
) {
    validate_active_interactions(world, intents);

    let mut ordered = intents.to_vec();
    ordered.sort_by_key(|(player, _)| *player);
    for (player, intent) in ordered {
        if intent.climb_pressed {
            try_climb(world, player);
        }
        if intent.interact_pressed {
            handle_interact_press(world, player);
        }
    }

    advance_holds(world, dt);
}

pub fn prompt_for_player(world: &InteractionWorld, player: PlayerId) -> InteractionPrompt {
    let Some(actor) = world.player(player) else {
        return InteractionPrompt {
            target: None,
            text: "PLAYER MISSING".to_string(),
        };
    };

    if let Some(target) = actor.active_target
        && let Some(object) = world.object(target)
    {
        return InteractionPrompt {
            target: Some(target),
            text: format!("HOLDING • {} • {}", object.name, object.kind.state_label()),
        };
    }

    if let Some(equipment) = actor.carrying {
        if let Some(target) = nearest_socket(world, actor.position, equipment) {
            return InteractionPrompt {
                target: Some(target),
                text: format!("E • SOCKET EQUIPMENT {}", equipment.0),
            };
        }
        return InteractionPrompt {
            target: None,
            text: format!("E • DROP EQUIPMENT {}", equipment.0),
        };
    }

    let Some(target) = nearest_target(world, actor.position) else {
        return InteractionPrompt {
            target: None,
            text: "NO INTERACTION IN RANGE".to_string(),
        };
    };
    let object = world.object(target).expect("target exists");
    let action = match &object.kind {
        InteractionKind::Lever { .. } => "E • ACTIVATE",
        InteractionKind::Door { requires_lever, .. } => {
            if requires_lever.is_some_and(|lever| !lever_is_active(world, lever)) {
                "E • OPERATE (NO POWER)"
            } else {
                "E • OPERATE"
            }
        }
        InteractionKind::TimedControl { .. } => {
            if let Some(owner) = object.active_users.first() {
                return InteractionPrompt {
                    target: Some(target),
                    text: format!(
                        "OCCUPIED BY {} • {}",
                        owner.label(),
                        object.kind.state_label()
                    ),
                };
            }
            "HOLD E • EXCLUSIVE CONTROL"
        }
        InteractionKind::TwoPlayerControl { .. } => "HOLD E • JOIN SHARED CONTROL",
        InteractionKind::Carryable { location, .. } => match location {
            ItemLocation::Ground(_) => "E • PICK UP",
            ItemLocation::Carried(_) | ItemLocation::Socketed(_) => "UNAVAILABLE",
        },
        InteractionKind::EquipmentSocket {
            inserted,
            operations,
            ..
        } => {
            if inserted.is_none() {
                "SOCKET EMPTY"
            } else if *operations == 0 {
                "E • OPERATE SOCKETED EQUIPMENT"
            } else {
                "E • RECOVER EQUIPMENT"
            }
        }
        InteractionKind::ClimbPoint { .. } => "Q • CLIMB",
    };
    InteractionPrompt {
        target: Some(target),
        text: format!("{action} • {}", object.name),
    }
}

fn validate_active_interactions(
    world: &mut InteractionWorld,
    intents: &[(PlayerId, PlayerIntent)],
) {
    let active = world
        .players
        .iter()
        .filter_map(|player| player.active_target.map(|target| (player.id, target)))
        .collect::<Vec<_>>();

    for (player, target) in active {
        let held = intent_for(intents, player).is_some_and(|intent| intent.interact_held);
        let in_range = player_in_range(world, player, target);
        if !held || !in_range {
            interrupt(world, player, target);
        }
    }
}

fn handle_interact_press(world: &mut InteractionWorld, player: PlayerId) {
    let Some(actor) = world.player(player).copied() else {
        return;
    };
    if actor.active_target.is_some() {
        return;
    }

    if let Some(equipment) = actor.carrying {
        if let Some(socket_target) = nearest_socket(world, actor.position, equipment) {
            socket_equipment(world, player, equipment, socket_target);
        } else {
            drop_equipment(world, player, equipment);
        }
        return;
    }

    let Some(target) = nearest_target(world, actor.position) else {
        return;
    };
    let kind = world.object(target).map(|object| object.kind.clone());
    match kind {
        Some(InteractionKind::Lever { .. }) => activate_lever(world, player, target),
        Some(InteractionKind::Door { .. }) => operate_door(world, player, target),
        Some(InteractionKind::TimedControl { .. })
        | Some(InteractionKind::TwoPlayerControl { .. }) => start_hold(world, player, target),
        Some(InteractionKind::Carryable {
            equipment,
            location: ItemLocation::Ground(_),
        }) => pick_up(world, player, target, equipment),
        Some(InteractionKind::EquipmentSocket {
            inserted: Some(equipment),
            socket,
            operations,
            ..
        }) => {
            if operations == 0 {
                operate_socket(world, player, target);
            } else {
                recover_equipment(world, player, target, equipment, socket);
            }
        }
        _ => {}
    }
}

fn activate_lever(world: &mut InteractionWorld, player: PlayerId, target: InteractionId) {
    if let Some(object) = world.object_mut(target)
        && let InteractionKind::Lever { active } = &mut object.kind
    {
        *active = !*active;
        world.push_event(InteractionEvent::Activated { player, target });
    }
}

fn operate_door(world: &mut InteractionWorld, player: PlayerId, target: InteractionId) {
    let requirement = world.object(target).and_then(|object| match object.kind {
        InteractionKind::Door { requires_lever, .. } => requires_lever,
        _ => None,
    });
    if requirement.is_some_and(|lever| !lever_is_active(world, lever)) {
        world.push_event(InteractionEvent::Denied {
            player,
            target,
            reason: "power lever inactive",
        });
        return;
    }

    if let Some(object) = world.object_mut(target)
        && let InteractionKind::Door { open, .. } = &mut object.kind
    {
        *open = !*open;
        world.push_event(InteractionEvent::Operated { player, target });
    }
}

fn start_hold(world: &mut InteractionWorld, player: PlayerId, target: InteractionId) {
    let Some(policy) = world.object(target).map(|object| object.policy) else {
        return;
    };
    let occupied = world
        .object(target)
        .is_some_and(|object| !object.active_users.is_empty());
    if policy == InteractionPolicy::ExclusiveHold && occupied {
        world.push_event(InteractionEvent::Denied {
            player,
            target,
            reason: "exclusive control occupied",
        });
        return;
    }

    if let Some(actor) = world.player_mut(player) {
        actor.active_target = Some(target);
    }
    if let Some(object) = world.object_mut(target) {
        let joined = !object.active_users.is_empty();
        object.active_users.insert(player);
        world.push_event(if joined {
            InteractionEvent::Joined { player, target }
        } else {
            InteractionEvent::Started { player, target }
        });
    }
}

fn interrupt(world: &mut InteractionWorld, player: PlayerId, target: InteractionId) {
    if let Some(actor) = world.player_mut(player) {
        actor.active_target = None;
    }
    if let Some(object) = world.object_mut(target) {
        object.active_users.remove(&player);
        let reset_progress = match object.policy {
            InteractionPolicy::ExclusiveHold => true,
            InteractionPolicy::SharedHold { minimum_users } => {
                object.active_users.len() < minimum_users
            }
            InteractionPolicy::Instant => false,
        };
        if reset_progress {
            set_progress(&mut object.kind, 0.0);
        }
    }
    world.push_event(InteractionEvent::Interrupted { player, target });
}

fn advance_holds(world: &mut InteractionWorld, dt: f32) {
    let mut completed = Vec::new();
    for object in &mut world.objects {
        let should_advance = match object.policy {
            InteractionPolicy::ExclusiveHold => object.active_users.len() == 1,
            InteractionPolicy::SharedHold { minimum_users } => {
                object.active_users.len() >= minimum_users
            }
            InteractionPolicy::Instant => false,
        };
        if !should_advance {
            continue;
        }

        if advance_progress(&mut object.kind, dt) {
            completed.push(object.id);
        }
    }

    for target in completed {
        let users = world
            .object(target)
            .map(|object| object.active_users.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for player in users {
            if let Some(actor) = world.player_mut(player) {
                actor.active_target = None;
            }
        }
        if let Some(object) = world.object_mut(target) {
            object.active_users.clear();
        }
        world.push_event(InteractionEvent::Completed { target });
    }
}

fn advance_progress(kind: &mut InteractionKind, dt: f32) -> bool {
    match kind {
        InteractionKind::TimedControl {
            duration,
            progress,
            completions,
        }
        | InteractionKind::TwoPlayerControl {
            duration,
            progress,
            completions,
        } => {
            *progress += dt;
            if *progress >= *duration {
                *progress = 0.0;
                *completions += 1;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

fn set_progress(kind: &mut InteractionKind, value: f32) {
    match kind {
        InteractionKind::TimedControl { progress, .. }
        | InteractionKind::TwoPlayerControl { progress, .. } => *progress = value,
        _ => {}
    }
}

fn pick_up(
    world: &mut InteractionWorld,
    player: PlayerId,
    target: InteractionId,
    equipment: EquipmentId,
) {
    if let Some(actor) = world.player_mut(player) {
        actor.carrying = Some(equipment);
    }
    if let Some(object) = world.object_mut(target)
        && let InteractionKind::Carryable { location, .. } = &mut object.kind
    {
        *location = ItemLocation::Carried(player);
    }
    world.push_event(InteractionEvent::PickedUp { player, equipment });
}

fn drop_equipment(world: &mut InteractionWorld, player: PlayerId, equipment: EquipmentId) {
    let Some(position) = world.player(player).map(|actor| actor.position) else {
        return;
    };
    if let Some(actor) = world.player_mut(player) {
        actor.carrying = None;
    }
    if let Some(object) = carryable_mut(world, equipment) {
        object.position = position;
        if let InteractionKind::Carryable { location, .. } = &mut object.kind {
            *location = ItemLocation::Ground(position);
        }
    }
    world.push_event(InteractionEvent::Dropped { player, equipment });
}

fn socket_equipment(
    world: &mut InteractionWorld,
    player: PlayerId,
    equipment: EquipmentId,
    target: InteractionId,
) {
    let socket = world.object(target).and_then(|object| match object.kind {
        InteractionKind::EquipmentSocket {
            socket,
            accepts,
            inserted: None,
            ..
        } if accepts == equipment => Some(socket),
        _ => None,
    });
    let Some(socket) = socket else {
        return;
    };

    if let Some(actor) = world.player_mut(player) {
        actor.carrying = None;
    }
    if let Some(object) = world.object_mut(target)
        && let InteractionKind::EquipmentSocket { inserted, .. } = &mut object.kind
    {
        *inserted = Some(equipment);
    }
    if let Some(object) = carryable_mut(world, equipment)
        && let InteractionKind::Carryable { location, .. } = &mut object.kind
    {
        *location = ItemLocation::Socketed(socket);
    }
    world.push_event(InteractionEvent::Socketed {
        player,
        equipment,
        socket,
    });
}

fn operate_socket(world: &mut InteractionWorld, player: PlayerId, target: InteractionId) {
    if let Some(object) = world.object_mut(target)
        && let InteractionKind::EquipmentSocket { operations, .. } = &mut object.kind
    {
        *operations += 1;
        world.push_event(InteractionEvent::Operated { player, target });
    }
}

fn recover_equipment(
    world: &mut InteractionWorld,
    player: PlayerId,
    target: InteractionId,
    equipment: EquipmentId,
    socket: SocketId,
) {
    if let Some(actor) = world.player_mut(player) {
        actor.carrying = Some(equipment);
    }
    if let Some(object) = world.object_mut(target)
        && let InteractionKind::EquipmentSocket {
            inserted,
            operations,
            ..
        } = &mut object.kind
    {
        *inserted = None;
        *operations = 0;
    }
    if let Some(object) = carryable_mut(world, equipment)
        && let InteractionKind::Carryable { location, .. } = &mut object.kind
    {
        *location = ItemLocation::Carried(player);
    }
    world.push_event(InteractionEvent::Recovered {
        player,
        equipment,
        socket,
    });
}

fn try_climb(world: &mut InteractionWorld, player: PlayerId) {
    let Some(position) = world.player(player).map(|actor| actor.position) else {
        return;
    };
    let target = world
        .objects
        .iter()
        .filter(|object| matches!(object.kind, InteractionKind::ClimbPoint { .. }))
        .filter(|object| position.distance(object.position) <= object.radius)
        .min_by(|left, right| {
            position
                .distance_squared(left.position)
                .total_cmp(&position.distance_squared(right.position))
        })
        .map(|object| object.id);
    let Some(target) = target else {
        return;
    };

    if let Some(actor) = world.player_mut(player) {
        actor.climb_uses += 1;
        actor.position += Vec2::new(0.0, 82.0);
    }
    if let Some(object) = world.object_mut(target)
        && let InteractionKind::ClimbPoint { uses } = &mut object.kind
    {
        *uses += 1;
    }
    world.push_event(InteractionEvent::Climbed { player, target });
}

fn nearest_target(world: &InteractionWorld, position: Vec2) -> Option<InteractionId> {
    world
        .objects
        .iter()
        .filter(|object| object_is_targetable(object))
        .filter(|object| position.distance(object.position) <= object.radius)
        .min_by(|left, right| {
            position
                .distance_squared(left.position)
                .total_cmp(&position.distance_squared(right.position))
        })
        .map(|object| object.id)
}

fn nearest_socket(
    world: &InteractionWorld,
    position: Vec2,
    equipment: EquipmentId,
) -> Option<InteractionId> {
    world
        .objects
        .iter()
        .filter(|object| {
            matches!(
                object.kind,
                InteractionKind::EquipmentSocket {
                    accepts,
                    inserted: None,
                    ..
                } if accepts == equipment
            )
        })
        .filter(|object| position.distance(object.position) <= object.radius)
        .min_by(|left, right| {
            position
                .distance_squared(left.position)
                .total_cmp(&position.distance_squared(right.position))
        })
        .map(|object| object.id)
}

fn object_is_targetable(object: &super::model::InteractionObject) -> bool {
    match object.kind {
        InteractionKind::Carryable { location, .. } => {
            matches!(location, ItemLocation::Ground(_))
        }
        _ => true,
    }
}

fn player_in_range(world: &InteractionWorld, player: PlayerId, target: InteractionId) -> bool {
    let Some(actor) = world.player(player) else {
        return false;
    };
    let Some(object) = world.object(target) else {
        return false;
    };
    actor.position.distance(object.position) <= object.radius
}

fn lever_is_active(world: &InteractionWorld, lever: InteractionId) -> bool {
    world
        .object(lever)
        .is_some_and(|object| matches!(object.kind, InteractionKind::Lever { active: true }))
}

fn intent_for(intents: &[(PlayerId, PlayerIntent)], player: PlayerId) -> Option<PlayerIntent> {
    intents
        .iter()
        .find(|(candidate, _)| *candidate == player)
        .map(|(_, intent)| *intent)
}

fn carryable_mut(
    world: &mut InteractionWorld,
    equipment: EquipmentId,
) -> Option<&mut super::model::InteractionObject> {
    world.objects.iter_mut().find(|object| {
        matches!(
            object.kind,
            InteractionKind::Carryable {
                equipment: candidate,
                ..
            } if candidate == equipment
        )
    })
}
