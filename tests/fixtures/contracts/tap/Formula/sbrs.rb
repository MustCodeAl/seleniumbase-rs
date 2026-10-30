# A made-up formula, for `tests/contract_script.rs`: the script reads what its `install` and its
# `test do` rely on.
class Sbrs < Formula
  def install
    generate_completions_from_executable(bin/"sbrs", "completions", shells: [:bash, :nushell])
    (pkgshare/"warp-workflows").install Dir[".warp/workflows/*.yaml"]
  end

  test do
    assert_match "mkfile", shell_output("#{bin}/sbrs completions usage")
  end
end
