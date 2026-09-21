# Third-party notices

This repository's installers and release artefacts redistribute binaries and model weights
it did not produce. Their licences are reproduced here in full, which is a condition of
redistributing them at all, not a courtesy.

The licence covering this repository's **own** code is in `LICENSE.md`. It does not apply to
anything listed below, and nothing below is affected by it.

## whisper.cpp and ggml

The Windows installer **embeds** a compiled whisper.cpp engine under `moteur/`, and the
application downloads further engines (Vulkan, CUDA) at the user's request. All of them are
builds of whisper.cpp and ggml, which perform the local speech recognition.

Source: <https://github.com/ggml-org/whisper.cpp>

Distributed under the MIT licence, whose terms require the copyright notice and permission
notice below to travel with every copy, **including binary copies**.

```
MIT License

Copyright (c) 2023-2026 The ggml authors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Whisper models

The `ggml-*.bin` model files the application downloads are conversions of OpenAI's Whisper
models, released under the MIT licence.

Source: <https://github.com/openai/whisper>

```
MIT License

Copyright (c) 2022 OpenAI

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Microsoft Visual C++ runtime

The Windows installer places four Visual C++ runtime libraries (`vcruntime140.dll`,
`vcruntime140_1.dll`, `msvcp140.dll`, `vcomp140.dll`) beside the executable, because the
installer runs without elevation and cannot install a machine-wide redistributable.

They are redistributed under Microsoft's redistributable licence terms, which permit this
"app-local" deployment alongside an application.

Source: the Visual Studio redistributable directory, see
<https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files>

## Rust crates

Build-time and runtime dependencies are declared in `src-tauri/Cargo.toml` and locked in
`src-tauri/Cargo.lock`, overwhelmingly under MIT or Apache-2.0. They are not reproduced here
individually; `cargo tree` and the lockfile are the authoritative list.
