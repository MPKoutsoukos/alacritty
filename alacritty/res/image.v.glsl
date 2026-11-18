#if defined(GLES2_RENDERER)
attribute vec2 aPos;
attribute vec2 aTexCoords;

varying mediump vec2 texCoords;
#else
in vec2 aPos;
in vec2 aTexCoords;

out vec2 texCoords;
#endif

void main() {
    texCoords = aTexCoords;
    gl_Position = vec4(aPos, 0.0, 1.0);
}
