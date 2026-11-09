import { mount } from 'svelte';
import ViewerApp from './ViewerApp.svelte';
import '../app.css';

const app = mount(ViewerApp, {
  target: document.getElementById('viewer-app'),
});

export default app;
