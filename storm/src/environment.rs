use image::DynamicImage;

use crate::{DenseEntry, Id, Storm, storage::SetEntry};

pub struct Environment {
    id: Id<Environment>,
    pub name: String,
}

impl DenseEntry for Environment {
    type Key = Environment;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

pub struct EnvironmentDescriptor {
    name: Option<String>,
}

impl SetEntry for Environment {
    type Descriptor = EnvironmentDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        Self { id, name }
    }
}

pub struct EnvironmentBuilder<'a, 's> {
    name: Option<String>,
    storm: &'s mut Storm,
    source: Source<'a>,
}

impl<'a, 's> EnvironmentBuilder<'a, 's> {
    pub fn new(storm: &'s mut Storm) -> Self {
        Self {
            name: None,
            storm,
            source: Source::None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn from_equirectangular_map(mut self, radiance_image: &'a DynamicImage) -> Self {
        self.source = Source::EquirectangularMap(radiance_image);
        self
    }

    pub fn build(self, encoder: &'a mut wgpu::CommandEncoder) -> &'s mut Environment {
        let name = self.name.as_ref().map_or("", |name| name);
        match self.source {
            Source::None => (),
            Source::EquirectangularMap(radiance_image) => {
                let _hdr_texture = self
                    .storm
                    .texture_builder()
                    .name(&format!("{name} hdr texture"))
                    .from_dynamic_image(radiance_image, false)
                    .build(encoder);
            }
        }

        self.storm
            .environments
            .push(EnvironmentDescriptor { name: self.name })
    }
}

enum Source<'a> {
    None,
    EquirectangularMap(&'a DynamicImage),
}
