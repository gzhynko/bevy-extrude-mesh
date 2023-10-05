use std::ops::Neg;
use bevy::prelude::*;
use lerp::num_traits::FromPrimitive;

const DEFAULT_LEN: usize = 100;

#[derive(Clone, Debug)]
pub struct BezierCurve {
    points: Vec<Vec3>,
    sampled_lengths: Vec<f32>,

    arc_lengths: Vec<f32>,
    len: usize,
    length: f32,
}

impl BezierCurve {
    pub fn new(points: Vec<Vec3>, len: Option<usize>) -> Self {
        let mut curve = Self {
            points,
            sampled_lengths: Vec::new(),

            arc_lengths: vec![0.; len.unwrap_or(DEFAULT_LEN) + 1],
            len: len.unwrap_or(DEFAULT_LEN),
            length: 0.,
        };
        curve.generate_samples();

        curve
    }

    fn generate_samples(&mut self) {
        let mut prev_point = self.points[0];
        let mut pt: Vec3;
        let mut total = 0.;

        let mut samples = vec![0f32; 10];
        let step = 1. / 10.;
        let mut f = step;
        while f < 1. {
            pt = self.get_point(f).0;
            total += (pt - prev_point).length();
            samples.push(total);

            prev_point = pt;
            f += step;
        }

        pt = self.get_point(1.).0;
        samples.push(total + (pt - prev_point).length());
        self.sampled_lengths = samples;
    }

    fn calculate_point(&self, t: f32, t2: f32, t3: f32, it: f32, it2: f32, it3: f32) -> Vec3 {
        self.points[0] * (it3) +
            self.points[1] * (3. * it2 * t) +
            self.points[2] * (3. * it * t2) +
            self.points[3] * t3
    }

    fn calculate_normal(&self, tangent: Vec3, up: Vec3) -> Vec3 {
        let binormal = Vec3::cross(up, tangent);
        Vec3::cross(tangent, binormal)
    }

    fn calculate_tangent(&self, t: f32, t2: f32, it2: f32) -> Vec3 {
        (self.points[0] * -1. * it2 +
            self.points[1] * (t * (3. * t - 4.) + 1.) +
            self.points[2] * (-3. * t2 + t * 2.) +
            self.points[3] * t2).normalize()
    }

    fn get_point_pos_only(&self, t: f32) -> Vec3 {
        let t2 = t * t;
        let t3 = t2 * t;
        let it = 1. - t;
        let it2 = it * it;
        let it3 = it * it * it;

        self.calculate_point(t, t2, t3, it, it2, it3)
    }

    fn get_point(&self, t: f32) -> (Vec3, Vec3, Vec3, Quat) {
        let t2 = t * t;
        let t3 = t2 * t;
        let it = 1. - t;
        let it2 = it * it;
        let it3 = it * it * it;

        let tangent = self.calculate_tangent(t, t2, it2);
        let normal = self.calculate_normal(tangent, Vec3::Y);

        let f = tangent.normalize();
        let r = Vec3::cross(f, normal).normalize();
        let u = Vec3::cross(r, f);
        let orientation = Quat::from_mat3(&Mat3::from_cols(r, u, f.neg()));

        let point = self.calculate_point(t, t2, t3, it, it2, it3);

        (point, tangent, normal, orientation)
    }

    pub fn get_oriented_point(&self, t: f32) -> OrientedPoint {
        let (point, _, _, orientation) = self.get_point(t);

        OrientedPoint::new(point, orientation, self.sample(t))
    }

    pub fn generate_path(&self, subdivisions: u32) -> Vec<OrientedPoint> {
        let step = 1. / subdivisions as f32;
        let mut result = Vec::new();

        let mut i = 0.;
        while i < 1. {
            result.push(self.get_oriented_point(i));
            i += step;
        }

        result.push(self.get_oriented_point(1.));

        result
    }

    pub fn generate_path_with_custom_height_function<F: Fn(f64, f64) -> f64>(&self, subdivisions: u32, custom_height_function: F) -> Vec<OrientedPoint> {
        let step = 1. / subdivisions as f32;
        let mut result = Vec::new();

        let mut i = 0.;
        while i < 1. {
            let mut point = self.get_oriented_point(i);
            point.position.y = custom_height_function(point.position.x as f64, point.position.z as f64) as f32;
            result.push(point);
            i += step;
        }

        let mut final_point = self.get_oriented_point(1.);
        final_point.position.y = custom_height_function(final_point.position.x as f64, final_point.position.z as f64) as f32;
        result.push(final_point);

        result
    }

    pub fn calculate_arc_lengths_with_custom_height_function<F: Fn(f64, f64) -> f64>(&mut self, custom_height_function: &F) {
        let mut old_point = self.get_point_pos_only(0.);
        old_point.y = custom_height_function(old_point.x as f64, old_point.z as f64) as f32;
        let mut clen = 0.;

        for i in 1..=self.len {
            let mut point = self.get_point_pos_only(i as f32 / self.len as f32);
            point.y = custom_height_function(point.x as f64, point.z as f64) as f32;
            let (dx, dy, dz) = (old_point.x - point.x, old_point.y - point.y, old_point.z - point.z);
            clen += (dx * dx + dy * dy + dz * dz).sqrt();
            self.arc_lengths[i] = clen;
            old_point = point;
        }

        self.length = clen;
    }

    pub fn calculate_arc_lengths(&mut self) {
        let mut old_point = self.get_point_pos_only(0.);
        let mut clen = 0.;

        for i in 1..=self.len {
            let point = self.get_point_pos_only(i as f32 / self.len as f32);
            let (dx, dy, dz) = (old_point.x - point.x, old_point.y - point.y, old_point.z - point.z);
            clen += (dx * dx + dy * dy + dz * dz).sqrt();
            self.arc_lengths[i] = clen;
            old_point = point;
        }

        self.length = clen;
    }

    pub fn map(&self, u: f32) -> f32 {
        let target_length = u * self.arc_lengths[self.len];
        let mut low = 0;
        let mut high = self.len;
        let mut index = 0;
        while low < high {
            index = low + (((high - low) / 2) | 0);
            if self.arc_lengths[index] < target_length {
                low = index + 1;
            } else {
                high = index;
            }
        }
        if self.arc_lengths[index] > target_length {
            index -= 1;
        }

        let length_before = self.arc_lengths[index];
        if length_before == target_length {
            index as f32 / self.len as f32
        } else {
            (index as f32 + (target_length - length_before) / (self.arc_lengths[index + 1] - length_before)) / self.len as f32
        }
    }

    pub fn sample(&self, t: f32) -> f32 {
        let len = self.sampled_lengths.len();
        if len == 1 {
            return self.sampled_lengths[0];
        }

        let f = t * (len - 1) as f32;
        let id_lower = i32::from_f32(f.floor()).unwrap();
        let id_upper = i32::from_f32(f.ceil()).unwrap();

        if id_upper >= len as i32 {
            return self.sampled_lengths[len - 1];
        }
        if id_lower < 0 {
            return self.sampled_lengths[0];
        }

        lerp::Lerp::lerp(self.sampled_lengths[id_lower as usize], self.sampled_lengths[id_upper as usize], f - id_lower as f32)
    }
}

#[derive(Debug, Clone, Default)]
pub struct OrientedPoint {
    pub position: Vec3,
    pub rotation: Quat,
    pub v_coordinate: f32, // the V of the UV coordinates
}

impl OrientedPoint {
    pub fn new(position: Vec3, rotation: Quat, v_coordinate: f32) -> Self {
        Self {
            position,
            rotation,
            v_coordinate,
        }
    }

    pub fn local_to_world(&self, point: Vec3) -> Vec3 {
        self.position + self.rotation * point
    }

    pub fn world_to_local(&self, point: Vec3) -> Vec3 {
        self.rotation.inverse() * (point - self.position)
    }

    pub fn local_to_world_direction(&self, dir: Vec3) -> Vec3 {
        self.rotation * dir
    }
}
