#if defined(GLES2_RENDERER)
precision mediump float;
#define FRAG_COLOR gl_FragColor
#define TEXTURE texture2D

varying vec2 texCoords;
#else
out vec4 FragColor;
#define FRAG_COLOR FragColor
#define TEXTURE texture

in vec2 texCoords;
#endif

// Premultiplied RGBA.
uniform sampler2D image;
uniform float opacity;

void main() {
    FRAG_COLOR = TEXTURE(image, texCoords) * opacity;
}
