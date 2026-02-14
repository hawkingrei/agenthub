def _workspace_shell_build_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".stamp")
    tool = ctx.file.tool

    inputs = [tool]
    for dep in ctx.attr.data:
        inputs.extend(dep[DefaultInfo].files.to_list())

    ctx.actions.run_shell(
        inputs = inputs,
        outputs = [output],
        arguments = [tool.path, ctx.attr.workspace_dir, output.path],
        command = """
set -euo pipefail
workspace="$2"
if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
  workspace="${BUILD_WORKSPACE_DIRECTORY}"
fi
bash "$1" "${workspace}"
touch "$3"
""",
        mnemonic = "WorkspaceShellBuild",
        use_default_shell_env = True,
    )

    return [DefaultInfo(files = depset([output]))]

def _workspace_shell_test_impl(ctx):
    tool = ctx.file.tool
    test_script = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        output = test_script,
        is_executable = True,
        content = """#!/usr/bin/env bash
set -euo pipefail
tool_rel="%s"
workspace="${BUILD_WORKSPACE_DIRECTORY:-}"

if [[ -z "${workspace}" ]]; then
  candidates=(
    "${TEST_SRCDIR:-}/${TEST_WORKSPACE:-}"
    "${TEST_SRCDIR:-}/_main"
    "${TEST_SRCDIR:-}/__main__"
  )
  for candidate in "${candidates[@]}"; do
    if [[ -f "${candidate}/${tool_rel}" ]]; then
      workspace="${candidate}"
      break
    fi
  done
fi

if [[ -z "${workspace}" ]]; then
  echo "failed to locate workspace for ${tool_rel}" >&2
  exit 1
fi

bash "${workspace}/${tool_rel}" "${workspace}"
""" % tool.short_path,
    )

    runfiles = ctx.runfiles(files = [tool])
    for dep in ctx.attr.data:
        info = dep[DefaultInfo]
        runfiles = runfiles.merge(info.default_runfiles)
        runfiles = runfiles.merge(ctx.runfiles(files = info.files.to_list()))

    return [DefaultInfo(executable = test_script, runfiles = runfiles)]

workspace_shell_build = rule(
    implementation = _workspace_shell_build_impl,
    attrs = {
        "tool": attr.label(allow_single_file = True, mandatory = True),
        "data": attr.label_list(allow_files = True),
        "workspace_dir": attr.string(default = "."),
    },
)

workspace_shell_test = rule(
    implementation = _workspace_shell_test_impl,
    test = True,
    attrs = {
        "tool": attr.label(allow_single_file = True, mandatory = True),
        "data": attr.label_list(allow_files = True),
    },
)
