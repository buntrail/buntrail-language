interface Props { name: string }
export function Panel(props: Props) {
  return <section><h1>hello</h1><p>{props.name}</p></section>;
}
