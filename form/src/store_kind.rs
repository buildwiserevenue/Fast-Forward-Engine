// Generated file: it is replaced whole on every update, so a change made here is not kept.

//! Which store a title comes from, and whether its id names that title on
//! every machine.

/// The store a title comes from; its id is an opaque string in that store's
/// own format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Store {
    Steam,
    Epic,
    Gog,
    /// Microsoft Store / Game Pass on PC.
    Gdk,
    /// No store claims this title: its id names it on one machine only, so
    /// elsewhere it is named by its executable.
    Local,
}

impl Store {
    pub fn as_str(self) -> &'static str {
        match self {
            Store::Steam => "steam",
            Store::Epic => "epic",
            Store::Gog => "gog",
            Store::Gdk => "gdk",
            Store::Local => "local",
        }
    }

    /// The store a name spells, exactly as [`Store::as_str`] writes it.
    pub fn parse(name: &str) -> Option<Store> {
        Store::ALL.into_iter().find(|s| s.as_str() == name)
    }

    /// True when the id means the same title on every machine, so a report or
    /// a catalogue entry can name the title by it. A [`Store::Local`] id does
    /// not.
    pub fn is_portable(self) -> bool {
        !matches!(self, Store::Local)
    }

    /// Every store, once: what [`Store::parse`] can answer.
    pub const ALL: [Store; 5] = [
        Store::Steam,
        Store::Epic,
        Store::Gog,
        Store::Gdk,
        Store::Local,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_store_is_spelled_back_to_itself() {
        for s in Store::ALL {
            assert_eq!(Store::parse(s.as_str()), Some(s));
        }
        assert_eq!(Store::parse("itch"), None);
        assert_eq!(Store::parse("Steam"), None, "one spelling, the one written");
        assert_eq!(Store::parse(""), None);
    }

    #[test]
    fn only_a_machine_local_id_is_not_portable() {
        let portable: Vec<Store> = Store::ALL.into_iter().filter(|s| s.is_portable()).collect();
        assert_eq!(
            portable,
            [Store::Steam, Store::Epic, Store::Gog, Store::Gdk]
        );
    }
}
