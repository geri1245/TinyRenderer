# A small rendering engine using Rust and WebGPU

This repository was created as a learning project to implement some graphics algorithms using Rust and WebGPU.

## Features

- Rendering obj models
- Live shader recompilation
- Physically based rendering with HDR environment maps (only diffuse IBL is implemented)
- Point and directional lights
- Shadows
- Generating and displaying a skybox from an hdr map
- Basic post-processing (gamma correction, tone mapping)
- Basic level editor functionalities (adding models at runtime, deleting existing models, saving and loading levels)
- Runtime shader parameter setting

## In progress
- Screen space reflections (with HiZ tracing)
- gltf file loading

## Future plans

- Soft shadows
- Cascaded shadow maps
- Global illumination
- Grass rendering
- Water rendering
