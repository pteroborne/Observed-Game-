//! The logical interaction state machine: its [`model`] (players, objects, policies,
//! carryables, equipment sockets, climb points, events) and the pure [`engine`] tick
//! that resolves player intents against it. Re-exported flat for convenience.

pub mod engine;
pub mod model;

pub use engine::*;
pub use model::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use observed_core::{PlayerId, PlayerIntent};

    fn intent(player: u16, pressed: bool, held: bool, climb: bool) -> (PlayerId, PlayerIntent) {
        (
            PlayerId(player),
            PlayerIntent {
                interact_pressed: pressed,
                interact_held: held,
                climb_pressed: climb,
                ..Default::default()
            },
        )
    }

    fn place(world: &mut InteractionWorld, player: u16, target: u16) {
        let position = world.object(InteractionId(target)).unwrap().position;
        world.player_mut(PlayerId(player)).unwrap().position = position;
    }

    #[test]
    fn lever_activation_powers_door_operation() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 1);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert!(matches!(
            world.object(InteractionId(1)).unwrap().kind,
            InteractionKind::Door { open: false, .. }
        ));
        assert!(matches!(
            world.recent_events.last(),
            Some(InteractionEvent::Denied { .. })
        ));

        place(&mut world, 0, 0);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert!(matches!(
            world.object(InteractionId(0)).unwrap().kind,
            InteractionKind::Lever { active: true }
        ));

        place(&mut world, 0, 1);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert!(matches!(
            world.object(InteractionId(1)).unwrap().kind,
            InteractionKind::Door { open: true, .. }
        ));
    }

    #[test]
    fn exclusive_hold_denies_second_user_and_resets_when_interrupted() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 2);
        place(&mut world, 1, 2);
        tick_interactions(
            &mut world,
            &[intent(0, true, true, false), intent(1, true, true, false)],
            0.5,
        );

        let object = world.object(InteractionId(2)).unwrap();
        assert_eq!(object.active_users.len(), 1);
        assert!(object.active_users.contains(&PlayerId(0)));
        assert!(world.recent_events.iter().any(|event| matches!(
            event,
            InteractionEvent::Denied {
                player: PlayerId(1),
                ..
            }
        )));

        tick_interactions(
            &mut world,
            &[
                intent(0, false, false, false),
                intent(1, false, false, false),
            ],
            0.1,
        );
        assert!(matches!(
            world.object(InteractionId(2)).unwrap().kind,
            InteractionKind::TimedControl { progress, .. } if progress == 0.0
        ));
        assert!(world.recent_events.iter().any(|event| matches!(
            event,
            InteractionEvent::Interrupted {
                player: PlayerId(0),
                ..
            }
        )));
    }

    #[test]
    fn exclusive_hold_completes_after_duration() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 2);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.5);
        tick_interactions(&mut world, &[intent(0, false, true, false)], 0.5);
        tick_interactions(&mut world, &[intent(0, false, true, false)], 0.5);

        assert!(matches!(
            world.object(InteractionId(2)).unwrap().kind,
            InteractionKind::TimedControl {
                completions: 1,
                progress,
                ..
            } if progress == 0.0
        ));
        assert_eq!(world.player(PlayerId(0)).unwrap().active_target, None);
    }

    #[test]
    fn moving_out_of_range_interrupts_an_active_hold() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 2);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.4);
        world.player_mut(PlayerId(0)).unwrap().position = Vec2::new(600.0, 300.0);
        tick_interactions(&mut world, &[intent(0, false, true, false)], 0.1);

        assert_eq!(world.player(PlayerId(0)).unwrap().active_target, None);
        assert!(matches!(
            world.object(InteractionId(2)).unwrap().kind,
            InteractionKind::TimedControl { progress, .. } if progress == 0.0
        ));
        assert!(matches!(
            world.recent_events.last(),
            Some(InteractionEvent::Interrupted { .. })
        ));
    }

    #[test]
    fn shared_hold_requires_two_users_and_interrupts_below_quorum() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 3);
        place(&mut world, 1, 3);

        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.5);
        assert!(matches!(
            world.object(InteractionId(3)).unwrap().kind,
            InteractionKind::TwoPlayerControl { progress, .. } if progress == 0.0
        ));

        tick_interactions(
            &mut world,
            &[intent(0, false, true, false), intent(1, true, true, false)],
            0.5,
        );
        assert!(matches!(
            world.object(InteractionId(3)).unwrap().kind,
            InteractionKind::TwoPlayerControl { progress, .. } if progress > 0.49
        ));

        tick_interactions(
            &mut world,
            &[
                intent(0, false, false, false),
                intent(1, false, true, false),
            ],
            0.1,
        );
        assert!(matches!(
            world.object(InteractionId(3)).unwrap().kind,
            InteractionKind::TwoPlayerControl { progress, .. } if progress == 0.0
        ));
    }

    #[test]
    fn shared_hold_completes_with_two_simultaneous_users() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 3);
        place(&mut world, 1, 3);
        tick_interactions(
            &mut world,
            &[intent(0, true, true, false), intent(1, true, true, false)],
            0.65,
        );
        tick_interactions(
            &mut world,
            &[intent(0, false, true, false), intent(1, false, true, false)],
            0.65,
        );
        assert!(matches!(
            world.object(InteractionId(3)).unwrap().kind,
            InteractionKind::TwoPlayerControl { completions: 1, .. }
        ));
        assert_eq!(world.player(PlayerId(0)).unwrap().active_target, None);
        assert_eq!(world.player(PlayerId(1)).unwrap().active_target, None);
    }

    #[test]
    fn carryable_can_be_picked_up_carried_and_dropped() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 4);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert_eq!(
            world.player(PlayerId(0)).unwrap().carrying,
            Some(EquipmentId(0))
        );
        assert!(matches!(
            world.object(InteractionId(4)).unwrap().kind,
            InteractionKind::Carryable {
                location: ItemLocation::Carried(PlayerId(0)),
                ..
            }
        ));

        world.player_mut(PlayerId(0)).unwrap().position = Vec2::new(500.0, 300.0);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert_eq!(world.player(PlayerId(0)).unwrap().carrying, None);
        assert!(matches!(
            world.object(InteractionId(4)).unwrap().kind,
            InteractionKind::Carryable {
                location: ItemLocation::Ground(position),
                ..
            } if position == Vec2::new(500.0, 300.0)
        ));
    }

    #[test]
    fn equipment_can_be_socketed_operated_and_recovered() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 4);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        place(&mut world, 0, 5);
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert!(matches!(
            world.object(InteractionId(5)).unwrap().kind,
            InteractionKind::EquipmentSocket {
                inserted: Some(EquipmentId(0)),
                ..
            }
        ));

        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert!(matches!(
            world.object(InteractionId(5)).unwrap().kind,
            InteractionKind::EquipmentSocket { operations: 1, .. }
        ));

        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        assert_eq!(
            world.player(PlayerId(0)).unwrap().carrying,
            Some(EquipmentId(0))
        );
        assert!(matches!(
            world.object(InteractionId(5)).unwrap().kind,
            InteractionKind::EquipmentSocket { inserted: None, .. }
        ));
    }

    #[test]
    fn climb_uses_separate_climb_intent() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 6);
        let before = world.player(PlayerId(0)).unwrap().position.y;
        tick_interactions(&mut world, &[intent(0, false, false, true)], 0.01);
        assert_eq!(world.player(PlayerId(0)).unwrap().climb_uses, 1);
        assert!(world.player(PlayerId(0)).unwrap().position.y > before);
        assert!(matches!(
            world.object(InteractionId(6)).unwrap().kind,
            InteractionKind::ClimbPoint { uses: 1 }
        ));
    }

    #[test]
    fn prompts_are_contextual_and_unambiguous() {
        let mut world = InteractionWorld::authored_lab();
        place(&mut world, 0, 4);
        assert!(
            prompt_for_player(&world, PlayerId(0))
                .text
                .contains("PICK UP")
        );
        tick_interactions(&mut world, &[intent(0, true, true, false)], 0.01);
        world.player_mut(PlayerId(0)).unwrap().position = Vec2::new(500.0, 300.0);
        assert!(prompt_for_player(&world, PlayerId(0)).text.contains("DROP"));
        place(&mut world, 0, 5);
        assert!(
            prompt_for_player(&world, PlayerId(0))
                .text
                .contains("SOCKET")
        );
    }
}
