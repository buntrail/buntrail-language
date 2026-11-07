use buntrail_language::{DockerEscape, docker_escape};
#[test]
fn escape_directive_header_stops_at_real_boundaries() {
    assert_eq!(
        docker_escape("# syntax=docker/dockerfile:1\r\n# EsCaPe = `\r\nFROM alpine\r\n"),
        Ok(DockerEscape::Backtick)
    );
    for text in [
        "# ordinary comment\n# escape=`\n",
        "\n# escape=`\n",
        "FROM alpine\n# escape=`\n",
    ] {
        assert_eq!(docker_escape(text), Ok(DockerEscape::Backslash));
    }
    assert!(docker_escape("# escape=`\n# escape=\\\n").is_err());
    assert!(docker_escape("# escape=x\n").is_err());
    assert_eq!(
        docker_escape("# escape=\\\nFROM alpine\n"),
        Ok(DockerEscape::Backslash)
    );
}
