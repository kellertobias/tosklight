use crate::*;

pub fn resolve(
    values: impl IntoIterator<Item = TimedValue>,
) -> HashMap<AttributeAddress, AttributeValue> {
    let mut winners: HashMap<AttributeAddress, TimedValue> = HashMap::new();
    for candidate in values {
        let key = (candidate.fixture_id, candidate.attribute.clone());
        let replace = match winners.get(&key) {
            None => true,
            Some(current) if candidate.priority != current.priority => {
                candidate.priority > current.priority
            }
            Some(current) if candidate.merge_mode == MergeMode::Htp => {
                candidate.value.normalized().unwrap_or(0.0)
                    > current.value.normalized().unwrap_or(0.0)
            }
            Some(current) => candidate.changed_at > current.changed_at,
        };
        if replace {
            winners.insert(key, candidate);
        }
    }
    winners
        .into_iter()
        .map(|(key, value)| (key, value.value))
        .collect()
}
