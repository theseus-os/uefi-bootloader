#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    pub frame_buffer: FrameBuffer,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameBuffer {
    pub start: usize,
    pub info: FrameBufferInfo,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameBufferInfo {
    pub len: usize,
    pub width: usize,
    pub height: usize,
    pub pixel_format: PixelFormat,
    pub bytes_per_pixel: usize,
    pub stride: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    Rgb,
    Bgr,
}
