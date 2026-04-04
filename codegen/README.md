Code Generation
===============

The source code for the different `RingBuffer` variants can be generated
by running this command in the top directory:

    cargo run -p codegen

While working on the templates, it can be helpful to automatically run the code generation
whenever any of the templates changes.
You can do this with

    cargo run -p codegen -- --watch

If you change to the `codegen/` directory first, you can drop the `-p codegen` argument.

The templates can be found in the [templates] sub-directory
and the configuration files are in [configs].
