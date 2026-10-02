//! Prepared rank anchors shared by physical scalars and exact native integers.
//! Preparation is proportional to control points; evaluating one rank never allocates N values.
pub(super) struct SpreadRankLayout {
    count: usize,
    points: usize,
    anchors: Vec<(usize, usize)>,
}
impl SpreadRankLayout {
    pub fn new(points: usize, count: usize) -> Self {
        let mut anchors = Vec::new();
        if points >= 2 && count >= points {
            let denominator = (points - 1) as u128;
            for point in 0..points {
                let numerator = point as u128 * (count - 1) as u128;
                let item = (numerator / denominator) as usize;
                let remainder = numerator % denominator;
                if remainder == 0 {
                    anchors.push((item, point));
                } else if remainder * 2 == denominator {
                    anchors.extend([(item, point), (item + 1, point)]);
                } else {
                    anchors.push((item + usize::from(remainder * 2 > denominator), point));
                }
            }
        }
        Self {
            count,
            points,
            anchors,
        }
    }
    /// (left control point, right control point, numerator, denominator).
    pub fn weights(&self, rank: usize) -> (usize, usize, u128, u128) {
        if self.count <= 1 || self.points <= 1 {
            return (0, 0, 0, 1);
        }
        if self.points > self.count {
            let denominator = (self.count - 1) as u128;
            let position = rank as u128 * (self.points - 1) as u128;
            let left = (position / denominator) as usize;
            return (
                left,
                (left + 1).min(self.points - 1),
                position % denominator,
                denominator,
            );
        }
        let right = self.anchors.partition_point(|(item, _)| *item < rank);
        let (right_rank, right_point) = self.anchors[right];
        if right_rank == rank {
            return (right_point, right_point, 0, 1);
        }
        let (left_rank, left_point) = self.anchors[right - 1];
        (
            left_point,
            right_point,
            (rank - left_rank) as u128,
            (right_rank - left_rank) as u128,
        )
    }
    /// Match the established float index interpolation before mixing wider physical values.
    pub fn scalar_position(&self, rank: usize) -> f32 {
        if self.count <= 1 || self.points <= 1 {
            return 0.0;
        }
        if self.points > self.count {
            return rank as f32 * (self.points - 1) as f32 / (self.count - 1) as f32;
        }
        let (left, right, step, span) = self.weights(rank);
        (left as f32 * (span - step) as f32 + right as f32 * step as f32) / span as f32
    }
}
