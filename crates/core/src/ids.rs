macro_rules! id_type {
    ($name:ident, $inner:ty) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(pub $inner);

        impl From<$inner> for $name {
            #[inline]
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $inner {
            #[inline]
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

id_type!(PlayerId, u64);
id_type!(ClubId, u32);
id_type!(CompetitionId, u32);
id_type!(MatchId, u64);
id_type!(TransferId, u64);
id_type!(SourcePlayerId, u64);
id_type!(MetricId, u16);
id_type!(MetricGroupId, u16);
id_type!(AttributeId, u16);
id_type!(RoleId, u16);
id_type!(FeatureId, u32);
id_type!(SeasonId, u16);
id_type!(StringId, u32);
id_type!(ProvenanceId, u64);
id_type!(ModelVersion, u64);
id_type!(DatasetVersion, u64);
id_type!(FeatureSetVersion, u64);
id_type!(CalibrationVersion, u64);
