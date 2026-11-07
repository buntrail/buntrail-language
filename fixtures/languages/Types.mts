interface User { name: string }
type Box<T> = { value: T };
export function greet<T extends User>(user: T): string {
  return "hello " + user.name;
}
class Greeter {
  greet(user: User): string { return user.name; }
}
