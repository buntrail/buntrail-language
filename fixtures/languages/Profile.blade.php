@extends('layouts.app')
{{-- hello comment --}}
@section('content')
<x-card><x-slot:title>hello</x-slot:title>
@forelse($users as $user)
<p>{{ $user->name }}</p>{!! $html !!}
@empty<p>hello</p>@endforelse
</x-card>
@php function greet() { return 'hello'; } @endphp
<script>const name = '{{ $name }}';</script>
<style>.hello { color: {{ $color }}; }</style>
@endsection
