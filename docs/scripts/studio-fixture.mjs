import { WebSocketServer } from 'ws';

// A protocol fixture for browser interaction tests; never loaded by the site.
export async function startStudioFixture() {
  const server = new WebSocketServer({ port: 0, host: '127.0.0.1' });
  await new Promise(resolve => server.on('listening', resolve));
  const state = { calls: [], authenticated: 0, audioFrames: 0, failStart: false, empty: false };
  const instances = new Map();
  let nextId = 0;
  const parameters = ['Mix', 'Tone', 'Drive', 'Output'].map((title, id) => ({
    id, title, shortTitle: title, units: '%', stepCount: 0,
    defaultNormalizedValue: .5, flags: { hidden: false, readOnly: false }
  }));
  server.on('connection', socket => {
    let authorized = false;
    socket.on('message', (data, binary) => {
      if (binary) {
        if (authorized) { state.audioFrames++; socket.send(data, { binary: true }); }
        return;
      }
      const { id, method, params = {} } = JSON.parse(data.toString());
      state.calls.push({ method, params });
      let result = {};
      if (method === 'bridge.hello') {
        if (!authorized) state.authenticated++;
        authorized = true;
      } else if (!authorized) {
        socket.send(JSON.stringify({ id, error: { code: 4010, message: 'Session authorization required' } }));
        return;
      } else if (method === 'plugin.list' || method === 'plugin.scan') {
        result = { plugins: state.empty ? [] : [{
          pluginId: 'fixture-effect', format: 'vst3', name: 'Copper Saturation',
          vendor: 'WVST test fixture', path: '/fixture.vst3', metadataSource: 'bundle-name',
          classes: [{ classId: 'effect', name: 'Copper Saturation', subcategories: ['Fx'], category: 'Audio Module Class' }]
        }], failures: [] };
      } else if (method === 'instance.create') {
        result = { ...params, instanceId: ++nextId, streamId: nextId, latencySamples: 128,
          runtimeCapabilities: { parameters: true } };
        instances.set(nextId, result);
      } else if (method === 'instance.start') {
        if (state.failStart) {
          socket.send(JSON.stringify({ id, error: { code: 5000, message: 'Fixture start failure' } }));
          return;
        }
        result = { instance: instances.get(params.instanceId) };
      } else if (method === 'instance.parameters') result = { parameters };
      else if (method === 'instance.parameter.info') {
        const value = params.valueNormalized ?? .5;
        result = { valueNormalized: value, valueString: `${Math.round(value * 100)}%` };
      }
      socket.send(JSON.stringify({ id, result }));
    });
  });
  return {
    state, endpoint: `ws://127.0.0.1:${server.address().port}`,
    close: () => { for (const socket of server.clients) socket.terminate(); server.close(); }
  };
}
