struct Example {
  var name: String = "hello"
  func greet() -> String {
    return """
    hello
    world
    """
  }
}
extension Example {
  func label() -> String { name }
}
