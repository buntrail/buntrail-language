import { value } from './value.js';
export function greet(name) {
  return "hello " + name;
}
export class Greeter {
  greet(name) { return `hello ${name}`; }
}
