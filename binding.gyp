{
  "targets": [
    {
      "target_name": "tree_sitter_al_binding",
      "include_dirs": [
        "<!(node -e \"require('nan')\")",
        "src"
      ],
      "sources": [
        "bindings/node/binding.cc",
        "src/parser.c"
      ],
      "conditions": [
        ["OS!='win'", {
          "sources": [
            "src/scanner.c"
          ]
        }],
        ["OS=='win'", {
          "sources": [
            "src/scanner.c"
          ]
        }]
      ],
      "cflags_c": [
        "-std=c99"
      ],
      "cflags_cc": [
        "-std=c++14"
      ],
      "xcode_settings": {
        "CLANG_CXX_LANGUAGE_STANDARD": "c++14",
        "MACOSX_DEPLOYMENT_TARGET": "10.9",
        "OTHER_CFLAGS": [
          "-Wno-unused-variable",
          "-Wno-unused-parameter"
        ]
      },
      "msvs_settings": {
        "VCCLCompilerTool": {
          "ExceptionHandling": 1
        }
      },
      "conditions": [
        ["OS=='linux'", {
          "libraries": [
            "<(module_root_dir)/target/release/libscanner.a",
            "-lpthread",
            "-ldl"
          ]
        }],
        ["OS=='mac'", {
          "libraries": [
            "<(module_root_dir)/target/release/libscanner.a"
          ]
        }],
        ["OS=='win'", {
          "libraries": [
            "<(module_root_dir)/target/release/scanner.lib"
          ]
        }]
      ]
    }
  ]
}
