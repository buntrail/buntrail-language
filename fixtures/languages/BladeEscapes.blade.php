@verbatim
<div>{{ clientValue }}</div>
@endverbatim
@@if($visible)
<p>@{{ clientValue }}</p>
@custom('hello (quoted)')
@if($name === 'hello')<b>hello</b>@endif
