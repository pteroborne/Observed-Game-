#[cfg(test)]
mod tests {
    use crate::*;
    use observed_core::RoomId;
    use std::collections::HashSet;

    const MAP_LITERAL: &str = r#"
        [ROOMS]
        ROOM 0: Spawn, Slots: [0; 1]
        ROOM 1: Key, Slots: [0]
        ROOM 2: Exit, Slots: [0]

        [HALLWAYS]
        HALLWAY 0: T-Junction, Slots: [0; 1; 2]

        [CONNECTIONS]
        ROOM 0, SLOT 0 <-> HALLWAY 0, SLOT 0
        ROOM 1, SLOT 0 <-> HALLWAY 0, SLOT 1
        ROOM 2, SLOT 0 <-> HALLWAY 0, SLOT 2
    "#;

    #[test]
    fn test_ascii_import_and_validation() {
        let (rooms, hallways, links) = parse_ascii_map(MAP_LITERAL).unwrap();
        assert_eq!(rooms.len(), 3);
        assert_eq!(hallways.len(), 1);
        assert_eq!(links.len(), 3);

        // Validation passes on initial connected map
        assert!(validate_connectivity(&rooms, &hallways, &links).is_ok());

        // Modify links to disconnect Room 2 (Exit)
        let mut disconnected_links = links.clone();
        disconnected_links[2] = Link {
            a: ThresholdEndpoint::Room(RoomId(0), ThresholdSlotId(1)),
            b: ThresholdEndpoint::Room(RoomId(1), ThresholdSlotId(0)),
        };
        // Validation must fail because Exit is unreachable and Room 2 is cut off
        assert!(validate_connectivity(&rooms, &hallways, &disconnected_links).is_err());
    }

    #[test]
    fn test_decoherence_shuffling() {
        let (rooms, hallways, mut links) = parse_ascii_map(MAP_LITERAL).unwrap();
        let mut rng = SimpleRng(42);

        // All unobserved -> shuffle should succeed and remain valid
        let observed = HashSet::new();
        let success = shuffle_links(&rooms, &hallways, &mut links, &observed, &mut rng);
        assert!(success);
        assert!(validate_connectivity(&rooms, &hallways, &links).is_ok());
    }
}
