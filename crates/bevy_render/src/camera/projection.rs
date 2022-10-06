use std::marker::PhantomData;

use bevy_app::{App, CoreSchedule, CoreSet, Plugin, StartupSet};
use bevy_ecs::{prelude::*, reflect::ReflectComponent};
use bevy_math::Mat4;
use bevy_reflect::{
    std_traits::ReflectDefault, FromReflect, GetTypeRegistration, Reflect, ReflectDeserialize,
    ReflectSerialize,
};
use serde::{Deserialize, Serialize};

/// Adds [`Camera`](crate::camera::Camera) driver systems for a given projection type.
pub struct CameraProjectionPlugin<T: CameraProjection>(PhantomData<T>);

impl<T: CameraProjection> Default for CameraProjectionPlugin<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

/// Label for [`camera_system<T>`], shared across all `T`.
///
/// [`camera_system<T>`]: crate::camera::camera_system
#[derive(SystemSet, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CameraUpdateSystem;

impl<T: CameraProjection + Component + GetTypeRegistration> Plugin for CameraProjectionPlugin<T> {
    fn build(&self, app: &mut App) {
        app.register_type::<T>()
            .edit_schedule(CoreSchedule::Startup, |schedule| {
                schedule.configure_set(CameraUpdateSystem.in_set(StartupSet::PostStartup));
            })
            .configure_set(CameraUpdateSystem.in_base_set(CoreSet::PostUpdate))
            .add_startup_system(
                crate::camera::camera_system::<T>
                    .in_set(CameraUpdateSystem)
                    // We assume that each camera will only have one projection,
                    // so we can ignore ambiguities with all other monomorphizations.
                    // FIXME: Add an archetype invariant for this https://github.com/bevyengine/bevy/issues/1481.
                    .ambiguous_with(CameraUpdateSystem),
            )
            .add_system(
                crate::camera::camera_system::<T>
                    .in_set(CameraUpdateSystem)
                    // We assume that each camera will only have one projection,
                    // so we can ignore ambiguities with all other monomorphizations.
                    // FIXME: Add an archetype invariant for this https://github.com/bevyengine/bevy/issues/1481.
                    .ambiguous_with(CameraUpdateSystem),
            );
    }
}

/// Trait to control the projection matrix of a camera.
///
/// Components implementing this trait are automatically polled for changes, and used
/// to recompute the camera projection matrix of the [`Camera`] component attached to
/// the same entity as the component implementing this trait.
///
/// [`Camera`]: crate::camera::Camera
pub trait CameraProjection {
    fn get_projection_matrix(&self) -> Mat4;
    fn update(&mut self, width: f32, height: f32);
    fn far(&self) -> f32;
}

/// A configurable [`CameraProjection`] that can select its projection type at runtime.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub enum Projection {
    Perspective(PerspectiveProjection),
    Orthographic(OrthographicProjection),
}

impl From<PerspectiveProjection> for Projection {
    fn from(p: PerspectiveProjection) -> Self {
        Self::Perspective(p)
    }
}

impl From<OrthographicProjection> for Projection {
    fn from(p: OrthographicProjection) -> Self {
        Self::Orthographic(p)
    }
}

impl CameraProjection for Projection {
    fn get_projection_matrix(&self) -> Mat4 {
        match self {
            Projection::Perspective(projection) => projection.get_projection_matrix(),
            Projection::Orthographic(projection) => projection.get_projection_matrix(),
        }
    }

    fn update(&mut self, width: f32, height: f32) {
        match self {
            Projection::Perspective(projection) => projection.update(width, height),
            Projection::Orthographic(projection) => projection.update(width, height),
        }
    }

    fn far(&self) -> f32 {
        match self {
            Projection::Perspective(projection) => projection.far(),
            Projection::Orthographic(projection) => projection.far(),
        }
    }
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Perspective(Default::default())
    }
}

/// A 3D camera projection in which distant objects appear smaller than close objects.
#[derive(Component, Debug, Clone, Reflect, FromReflect)]
#[reflect(Component, Default)]
pub struct PerspectiveProjection {
    /// The vertical field of view (FOV) in radians.
    ///
    /// Defaults to a value of π/4 radians or 45 degrees.
    pub fov: f32,

    /// The aspect ratio (width divided by height) of the viewing frustum.
    ///
    /// Bevy's [`camera_system`](crate::camera::camera_system) automatically
    /// updates this value when the aspect ratio of the associated window changes.
    ///
    /// Defaults to a value of `1.0`.
    pub aspect_ratio: f32,

    /// The distance from the camera in world units of the viewing frustum's near plane.
    ///
    /// Objects closer to the camera than this value will not be visible.
    ///
    /// Defaults to a value of `0.1`.
    pub near: f32,

    /// The distance from the camera in world units of the viewing frustum's far plane.
    ///
    /// Objects farther from the camera than this value will not be visible.
    ///
    /// Defaults to a value of `1000.0`.
    pub far: f32,
}

impl CameraProjection for PerspectiveProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_infinite_reverse_rh(self.fov, self.aspect_ratio, self.near)
    }

    fn update(&mut self, width: f32, height: f32) {
        self.aspect_ratio = width / height;
    }

    fn far(&self) -> f32 {
        self.far
    }
}

impl Default for PerspectiveProjection {
    fn default() -> Self {
        PerspectiveProjection {
            fov: std::f32::consts::PI / 4.0,
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 1.0,
        }
    }
}

// TODO: make this a component instead of a property
#[derive(Debug, Clone, Reflect, FromReflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub enum WindowOrigin {
    /// Scale the view frustum from the center.
    ///
    /// When resizing the window, object in the center of the screen remain in the center, while opposite
    /// faces of the view frustum expand/shrink equally.
    Center,
    /// Scale the view frustum from the bottom left corner.
    ///
    /// When resizing the window, the top and right faces of the view frustum will move accordingly, keeping
    /// the bottom and left faces in place.
    BottomLeft,
}

#[derive(Debug, Clone, Reflect, FromReflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub enum ScalingMode {
    /// Manually specify left/right/top/bottom values.
    /// Ignore window resizing; the image will stretch.
    None,
    /// Match the window size. 1 world unit = 1 pixel.
    WindowSize,
    /// Keeping the aspect ratio while the axes can't be smaller than given minimum.
    /// Arguments are in world units.
    AutoMin { min_width: f32, min_height: f32 },
    /// Keeping the aspect ratio while the axes can't be bigger than given maximum.
    /// Arguments are in world units.
    AutoMax { max_width: f32, max_height: f32 },
    /// Keep vertical axis constant; resize horizontal with aspect ratio.
    /// The argument is the desired height of the viewport in world units.
    FixedVertical(f32),
    /// Keep horizontal axis constant; resize vertical with aspect ratio.
    /// The argument is the desired width of the viewport in world units.
    FixedHorizontal(f32),
}

/// Project a 3D space onto a 2D surface using parallel lines, i.e., unlike [`PerspectiveProjection`],
/// the size at which objects appear remain the same regardless of depth.
///
/// The volume contained in the projection is called the *view frustum*. Since the viewport is rectangular,
/// the view frustum is in the shape of a rectangular prism.
///
/// Note that the cross sectional area of the view frustum and the apparent size of objects are inversely proportional.
#[derive(Component, Debug, Clone, Reflect, FromReflect)]
#[reflect(Component, Default)]
pub struct OrthographicProjection {
    /// The distance of the left face of the view frustum relative to the camera, measured in world units.
    ///
    /// This value will be set automatically unless `scaling_mode` is set to [`ScalingMode::None`].
    pub left: f32,
    /// The distance of the right face of the view frustum relative to the camera, measured in world units.
    ///
    /// This value will be set automatically unless `scaling_mode` is set to [`ScalingMode::None`].
    pub right: f32,
    /// The distance the bottom face of the view frustum relative to the camera, measured in world units.
    ///
    /// This value will be set automatically unless `scaling_mode` is set to [`ScalingMode::None`].
    pub bottom: f32,
    /// The distance of the top face of the view frustum relative to the camera, measured in world units.
    ///
    /// This value will be set automatically unless `scaling_mode` is set to [`ScalingMode::None`].
    pub top: f32,
    /// The distance of the near clipping plane in world units.
    ///
    /// Objects closer than this will not be rendered.
    pub near: f32,
    /// The distance of the far clipping plane in world units.
    ///
    /// Objects further than this will not be rendered.
    pub far: f32,
    /// Specifies the origin of the scaling.
    ///
    /// This will not have any effect if `scaling_mode` is set to `ScalingMode::None`.
    pub window_origin: WindowOrigin,
    pub scaling_mode: ScalingMode,
    /// Scales the width and height of the view frustum.
    ///
    /// As `scale` increases, the apparent size of objects decreases, and vice versa.
    pub scale: f32,
}

impl CameraProjection for OrthographicProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            self.left * self.scale,
            self.right * self.scale,
            self.bottom * self.scale,
            self.top * self.scale,
            // NOTE: near and far are swapped to invert the depth range from [0,1] to [1,0]
            // This is for interoperability with pipelines using infinite reverse perspective projections.
            self.far,
            self.near,
        )
    }

    fn update(&mut self, width: f32, height: f32) {
        let (viewport_width, viewport_height) = match self.scaling_mode {
            ScalingMode::WindowSize => (width, height),
            ScalingMode::AutoMin {
                min_width,
                min_height,
            } => {
                // Compare Pixels of current width and minimal height and Pixels of minimal width with current height.
                // Then use bigger (min_height when true) as what it refers to (height when true) and calculate rest so it can't get under minimum.
                if width * min_height > min_width * height {
                    (width * min_height / height, min_height)
                } else {
                    (min_width, height * min_width / width)
                }
            }
            ScalingMode::AutoMax {
                max_width,
                max_height,
            } => {
                // Compare Pixels of current width and maximal height and Pixels of maximal width with current height.
                // Then use smaller (max_height when true) as what it refers to (height when true) and calculate rest so it can't get over maximum.
                if width * max_height < max_width * height {
                    (width * max_height / height, max_height)
                } else {
                    (max_width, height * max_width / width)
                }
            }
            ScalingMode::FixedVertical(viewport_height) => {
                (width * viewport_height / height, viewport_height)
            }
            ScalingMode::FixedHorizontal(viewport_width) => {
                (viewport_width, height * viewport_width / width)
            }
            ScalingMode::None => return,
        };

        match self.window_origin {
            WindowOrigin::Center => {
                let half_width = viewport_width / 2.0;
                let half_height = viewport_height / 2.0;
                self.left = -half_width;
                self.bottom = -half_height;
                self.right = half_width;
                self.top = half_height;

                if let ScalingMode::WindowSize = self.scaling_mode {
                    if self.scale == 1.0 {
                        self.left = self.left.floor();
                        self.bottom = self.bottom.floor();
                        self.right = self.right.floor();
                        self.top = self.top.floor();
                    }
                }
            }
            WindowOrigin::BottomLeft => {
                self.left = 0.0;
                self.bottom = 0.0;
                self.right = viewport_width;
                self.top = viewport_height;
            }
        }
    }

    fn far(&self) -> f32 {
        self.far
    }
}

impl Default for OrthographicProjection {
    fn default() -> Self {
        OrthographicProjection {
            left: -1.0,
            right: 1.0,
            bottom: -1.0,
            top: 1.0,
            near: 0.0,
            far: 1000.0,
            window_origin: WindowOrigin::Center,
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0,
        }
    }
}
