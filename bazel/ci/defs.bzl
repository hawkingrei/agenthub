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
bash "$1" "$2"
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
workspace="${TEST_SRCDIR}/${TEST_WORKSPACE}"
bash "${workspace}/%s" "${workspace}"
""" % tool.short_path,
    )

    runfiles = ctx.runfiles(files = [tool])
    for dep in ctx.attr.data:
        runfiles = runfiles.merge(dep[DefaultInfo].default_runfiles)

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
