//! `WorldCoordinate` as a save writes it: `<room>.<x>.<y>.<abstractNode>`, the room by
//! name (`INV.` in front of a coordinate of the Inv campaign).

/// A saved `WorldCoordinate`, its room still a name (the world resolves it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedCoordinate {
    pub room: String,
    pub x: i32,
    pub y: i32,
    /// `abstractNode`; a death position's age in cycles.
    pub node: i32,
}

impl SavedCoordinate {
    /// `WorldCoordinate.FromString`; `None` where the game's `int.Parse` throws.
    #[must_use]
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        let first = usize::from(parts.len() == 5 && parts[0] == "INV");
        let fields = parts.get(first..)?;
        Some(Self {
            room: (*fields.first()?).to_owned(),
            x: Self::int(fields, 1)?,
            y: Self::int(fields, 2)?,
            node: Self::int(fields, 3)?,
        })
    }

    fn int(fields: &[&str], i: usize) -> Option<i32> {
        fields.get(i)?.trim().parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rooms_by_name_and_the_inv_prefix() {
        assert_eq!(
            SavedCoordinate::from_string("SU_A22.3.4.6"),
            Some(SavedCoordinate {
                room: "SU_A22".to_owned(),
                x: 3,
                y: 4,
                node: 6
            })
        );
        assert_eq!(
            SavedCoordinate::from_string("INV.SU_C04.-1.2.0").map(|c| (c.room, c.x)),
            Some(("SU_C04".to_owned(), -1))
        );
        assert_eq!(SavedCoordinate::from_string("SU_A22.3"), None);
    }
}
