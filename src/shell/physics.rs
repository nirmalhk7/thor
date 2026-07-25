use glam::Vec2;

pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
    pub value: Vec2,
    pub target: Vec2,
    pub velocity: Vec2,
}

impl Spring {
    pub fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self {
            stiffness,
            damping,
            mass,
            value: Vec2::ZERO,
            target: Vec2::ZERO,
            velocity: Vec2::ZERO,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let force = (self.target - self.value) * self.stiffness;
        let damping_force = self.velocity * self.damping;
        let acceleration = (force - damping_force) / self.mass;
        self.velocity += acceleration * dt;
        self.value += self.velocity * dt;
    }
}
