use glam::Vec3;

#[derive(Debug, Copy, Clone)]
pub struct Line {
    pub position: Vec3,
    pub direction: Vec3,
}

impl Line {
    /// Evaluates the line where the parameter is equal to `value`
    pub fn evaluate(&self, value: f32) -> Vec3 {
        self.position + self.direction * value
    }

    /// Calcluates the closest points between 2 lines
    // Based on https://math.stackexchange.com/a/2217845
    pub fn distance(&self, other: &Line) -> (Vec3, Vec3) {
        // The difference vector is perpendicular to both lines' tangent vector
        let diff_vec = self.direction.cross(other.direction);

        // Calculating the distance if needed:
        // let shortest_distance = diff_vec.dot(self.position - other.position) / diff_vec.length();

        // (other.dir x diff_vec) * (other.pos - self.pos) / diff_vec * diff_vec
        let self_distance = other
            .direction
            .cross(diff_vec)
            .dot(other.position - self.position)
            / diff_vec.length_squared();

        let other_distance = self
            .direction
            .cross(diff_vec)
            .dot(other.position - self.position)
            / diff_vec.length_squared();

        let self_closest_point = self.evaluate(self_distance);
        let other_closest_point = other.evaluate(other_distance);

        (self_closest_point, other_closest_point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_VEC_DIFF: f32 = 0.000001;

    #[test]
    fn distance_between_lines() {
        let line1 = Line {
            position: Vec3::new(2., 6., -9.),
            direction: Vec3::new(3., 4., -4.).normalize(),
        };
        let line2 = Line {
            position: Vec3::new(-1., -2., 3.),
            direction: Vec3::new(2., -6., 1.).normalize(),
        };

        let (p1, p2) = line1.distance(&line2);
        assert!(p1.abs_diff_eq(
            Vec3 {
                x: -4.167919799498746,
                y: -2.223893065998329,
                z: -0.7761069340016708,
            },
            MAX_VEC_DIFF,
        ));

        assert!(p2.abs_diff_eq(
            Vec3 {
                x: -1.427736006683375,
                y: -0.7167919799498746,
                z: 2.786131996658312,
            },
            MAX_VEC_DIFF
        ));
    }
}
