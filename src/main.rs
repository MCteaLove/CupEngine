
use std::ffi::{c_void, CStr};

use anyhow::{bail, Result};
use ash::vk;
use raw_window_handle::HasDisplayHandle;

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

// ============================================================
// CONFIGURATION
// ============================================================
mod transform;
mod rigid_body;
const ENABLE_VALIDATION: bool = cfg!(debug_assertions);

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

// ============================================================
// VULKAN DEBUG CALLBACK
// ============================================================

unsafe extern "system" fn vulkan_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut c_void,
) -> vk::Bool32 {

    if callback_data.is_null() {
        return vk::FALSE;
    }

    let message_ptr = (*callback_data).p_message;

    if message_ptr.is_null() {
        return vk::FALSE;
    }

    let message = CStr::from_ptr(message_ptr)
        .to_string_lossy();

    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        eprintln!("[VULKAN ERROR] [{message_type:?}] {message}");
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        eprintln!("[VULKAN WARNING] [{message_type:?}] {message}");
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        println!("[VULKAN INFO] [{message_type:?}] {message}");
    } else {
        println!("[VULKAN VERBOSE] [{message_type:?}] {message}");
    }

    vk::FALSE
}

// ============================================================
// VULKAN CONTEXT
// ============================================================

struct VulkanContext {
    _entry: ash::Entry,
    instance: ash::Instance,

    debug_utils: Option<ash::ext::debug_utils::Instance>,
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl VulkanContext {

    fn new(
        display_handle: raw_window_handle::RawDisplayHandle,
    ) -> Result<Self> {

        // 1. Load Vulkan library.

        let entry = unsafe {
            ash::Entry::load()?
        };

        println!("[ENGINE] Vulkan library loaded");

        // 2. Check validation layers.

        let mut layer_names = Vec::new();

        if ENABLE_VALIDATION {

            let available_layers = unsafe {
                entry.enumerate_instance_layer_properties()?
            };

            let validation_available = available_layers.iter().any(|layer| {

                let name = unsafe {
                    CStr::from_ptr(layer.layer_name.as_ptr())
                };

                name == VALIDATION_LAYER
            });

            if !validation_available {
                bail!("VK_LAYER_KHRONOS_validation is not installed");
            }

            layer_names.push(VALIDATION_LAYER.as_ptr());

            println!("[ENGINE] Validation layer enabled");
        }

        // 3. Required extensions.

        let mut extension_names = unsafe {
            ash_window::enumerate_required_extensions(display_handle)?
        }.to_vec();

        if ENABLE_VALIDATION {
            extension_names.push(
                ash::ext::debug_utils::NAME.as_ptr()
            );
        }

        // 4. Application information.

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"RustEngine")
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(c"CustomEngine")
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_2);

        // 5. Create Vulkan Instance.

        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layer_names)
            .enabled_extension_names(&extension_names);

        let instance = unsafe {
            entry.create_instance(&instance_info, None)?
        };

        println!("[ENGINE] Vulkan Instance created");

        // 6. Create Debug Messenger.

        let mut debug_utils = None;

        let mut debug_messenger =
            vk::DebugUtilsMessengerEXT::null();

        if ENABLE_VALIDATION {

            let debug_info =
                vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    )
                    .pfn_user_callback(Some(vulkan_debug_callback));

            let loader = ash::ext::debug_utils::Instance::new(
                &entry,
                &instance,
            );

            match unsafe {
                loader.create_debug_utils_messenger(&debug_info, None)
            } {

                Ok(messenger) => {
                    debug_messenger = messenger;
                    debug_utils = Some(loader);

                    println!("[ENGINE] Debug Messenger created");
                }

                Err(error) => {
                    unsafe {
                        instance.destroy_instance(None);
                    }

                    return Err(error.into());
                }
            }
        }

        Ok(Self {
            _entry: entry,
            instance,
            debug_utils,
            debug_messenger,
        })
    }
}

// ============================================================
// RESOURCE DESTRUCTION
// ============================================================

impl Drop for VulkanContext {

    fn drop(&mut self) {

        unsafe {

            if let Some(loader) = &self.debug_utils {

                if self.debug_messenger !=
                    vk::DebugUtilsMessengerEXT::null()
                {
                    loader.destroy_debug_utils_messenger(
                        self.debug_messenger,
                        None,
                    );
                }
            }

            self.instance.destroy_instance(None);
        }

        println!("[ENGINE] Vulkan Context destroyed");
    }
}

// ============================================================
// APPLICATION
// ============================================================

struct App {
    window: Option<Window>,
    vulkan: Option<VulkanContext>,
}

impl App {

    fn new() -> Self {
        Self {
            window: None,
            vulkan: None,
        }
    }
}

impl ApplicationHandler for App {

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {

        if self.window.is_some() {
            return;
        }

        // Create window.

        let attributes = Window::default_attributes()
            .with_title("Custom Rust Engine")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));

        let window = match event_loop.create_window(attributes) {

            Ok(window) => window,

            Err(error) => {
                eprintln!("[ERROR] Window creation failed: {error}");
                event_loop.exit();
                return;
            }
        };

        println!("[ENGINE] Window created");

        // Obtain display handle.

        let display_handle = match window.display_handle() {

            Ok(handle) => handle.as_raw(),

            Err(error) => {
                eprintln!("[ERROR] Display handle failed: {error}");
                event_loop.exit();
                return;
            }
        };

        // Initialize Vulkan.

        let vulkan = match VulkanContext::new(display_handle) {

            Ok(context) => context,

            Err(error) => {
                eprintln!("[ERROR] Vulkan initialization failed: {error:#}");
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.vulkan = Some(vulkan);

        println!("[ENGINE] Initialization completed");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {

        match event {

            WindowEvent::CloseRequested => {
                println!("[ENGINE] Closing application");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                println!(
                    "[ENGINE] Window size: {}x{}",
                    size.width,
                    size.height
                );
            }

            _ => {}
        }
    }
}

// ============================================================
// MAIN
// ============================================================


fn main() -> anyhow::Result<()> {
    use glam::{Quat, Vec3};

    println!("================================");
    println!("       CUSTOM RUST ENGINE       ");
    println!("================================");

    // Создаём корабль.
    let mut ship = transform::Transform::new();

    println!("Initial position: {:?}", ship.position);
    println!("Forward: {:?}", ship.forward());

    // Проверяем Transform.
    ship.translate_local(Vec3::new(0.0, 0.0, -5.0));

    ship.rotate_local(Quat::from_rotation_y(
        90.0_f32.to_radians()
    ));

    println!("New position: {:?}", ship.position);
    println!("New forward: {:?}", ship.forward());

    // Создаём физическое тело.
    let mut physics = rigid_body::RigidBody::new(2.0);

    // Прикладываем силу.
    physics.apply_force(Vec3::new(0.0, 0.0, -10.0));

    // Прикладываем вращающий момент.
    physics.apply_torque(Vec3::new(0.0, 2.0, 0.0));

    // Выполняем один шаг симуляции.
    physics.update(&mut ship, 1.0 / 60.0);

    println!("--- PHYSICS TEST ---");
    println!("Position: {:?}", ship.position);
    println!("Velocity: {:?}", physics.linear_velocity);
    println!("Rotation: {:?}", ship.rotation);
    println!("Angular velocity: {:?}", physics.angular_velocity);

    // Только теперь запускаем оконный цикл.
    let event_loop = EventLoop::new()?;

    let mut app = App::new();

    event_loop.run_app(&mut app)?;

    Ok(())
}
