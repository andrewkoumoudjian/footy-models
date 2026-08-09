use footy_core::{ClubId, CompetitionId, Date, PlayerId, StringId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreferredFoot {
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Player {
    pub id: PlayerId,
    pub birth_date: Date,
    pub name: StringId,
    pub height_cm: Option<u16>,
    pub preferred_foot: Option<PreferredFoot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Club {
    pub id: ClubId,
    pub name: StringId,
    pub country: Option<StringId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Competition {
    pub id: CompetitionId,
    pub name: StringId,
    pub country: Option<StringId>,
}
