import React, { useEffect, useReducer, useRef, useState } from 'react';
import { PanResponder, Pressable, SafeAreaView, StatusBar, StyleSheet, Text, View } from 'react-native';
import { ExpectationDriver } from 'expectation-driver';
import { Kettle, PRESETS } from './src/kettle';

const C = {
  bg: '#1a1b17', panel: '#2b2d26', edge: '#45483d', screen: '#050806', text: '#dfe8df',
  muted: '#919c91', faint: '#3a403a', ok: '#4cff7a', caution: '#ffb21f', warning: '#ff5a4f', control: '#161a16',
};
const MONO = 'Menlo';
const TICK = 50;

/** The embedded Expectation driver: when a runner is listening, it drives this app. */
function startDriver(k: Kettle) {
  const driver = new ExpectationDriver({ platform: 'React Native', app: 'Kettle' })
    .observe('the kettle', () => k.phase)
    .observe('the dial', () => k.dial)
    .observe('the preset', () => PRESETS[k.preset])
    .observe('the child lock', () => (k.lock ? 'on' : 'off'))
    .set('the dial', v => { k.dial = Number(v); })
    .arrange('the kettle', v => k.force(String(v)))
    .arrange('the preset', v => { k.preset = Math.max(0, PRESETS.indexOf(v as (typeof PRESETS)[number])); })
    .arrange('the child lock', v => { k.lock = v === 'on'; })
    .on('the user presses start', () => k.start())
    .on('the user presses reset', () => k.reset())
    .on('the user presses preset', () => k.pressPreset())
    .on('the user presses lock', () => k.pressLock())
    .on('the water runs out', () => k.runOut())
    .onReset(() => k.restore());
  // A demo: always on. In a real app, start it only in development or test builds.
  driver.start('http://127.0.0.1:4477');
  return driver;
}

function Dial({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const [width, setWidth] = useState(1);
  const widthRef = useRef(1);
  widthRef.current = width;
  const set = (x: number) => onChange(Math.round(Math.min(Math.max(x / widthRef.current, 0), 1) * 20) * 5);
  const pan = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onMoveShouldSetPanResponder: () => true,
      onPanResponderGrant: e => set(e.nativeEvent.locationX),
      onPanResponderMove: e => set(e.nativeEvent.locationX),
    }),
  ).current;
  return (
    <View>
      <View style={styles.dialLabel}>
        <Text style={[styles.mono, { color: C.muted }]}>DIAL</Text>
        <Text style={[styles.mono, { color: C.text }]}>{value} %</Text>
      </View>
      <View style={styles.track} onLayout={e => setWidth(e.nativeEvent.layout.width)} {...pan.panHandlers}>
        <View pointerEvents="none" style={[styles.fill, { width: `${value}%` }]} />
        <View pointerEvents="none" style={[styles.mark, { left: '55%' }]} />
      </View>
    </View>
  );
}

function Button({ label, onPress }: { label: string; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} style={({ pressed }) => [styles.button, pressed && { backgroundColor: '#2a302a' }]}>
      <Text style={[styles.mono, styles.bold, { color: C.text }]}>{label}</Text>
    </Pressable>
  );
}

export default function App() {
  const kettle = useRef(new Kettle()).current;
  const [, render] = useReducer((n: number) => n + 1, 0);

  useEffect(() => {
    const driver = startDriver(kettle);
    const timer = setInterval(() => {
      kettle.tick(TICK / 1000);
      render();
    }, TICK);
    return () => {
      clearInterval(timer);
      driver.stop();
    };
  }, [kettle]);

  const act = (f: () => void) => () => {
    f();
    render();
  };
  const [state, color] = {
    off: ['OFF', C.text],
    starting: ['STARTING', C.caution],
    heating: ['HEATING', C.ok],
    dry: ['DRY', C.warning],
  }[kettle.phase];
  const lit = kettle.phase === 'heating' || kettle.phase === 'starting';
  const row = (label: string, value: string, c: string) => (
    <View style={styles.row}>
      <Text style={[styles.mono, styles.label]}>{label}</Text>
      <Text style={[styles.mono, styles.bold, { color: c }]}>{value}</Text>
    </View>
  );

  return (
    <SafeAreaView style={styles.root}>
      <StatusBar barStyle="light-content" />
      <View style={styles.panel}>
        <View style={styles.screen}>
          {row('PRESET', PRESETS[kettle.preset], C.text)}
          {row('STATE', state, color)}
          {row('WATER', `${Math.round(kettle.temperature)} °C`, C.text)}
          <View style={[styles.row, { marginTop: 6, alignItems: 'center' }]}>
            <View style={[styles.light, { backgroundColor: lit ? C.warning : C.faint }]} />
            <Text style={[styles.mono, styles.bold, styles.badge, { color: kettle.lock ? C.caution : C.faint, borderColor: kettle.lock ? C.caution : C.edge }]}>
              LOCK
            </Text>
          </View>
        </View>
        <Dial value={kettle.dial} onChange={v => { kettle.dial = v; render(); }} />
        <View style={styles.buttons}>
          <Button label="START" onPress={act(() => kettle.start())} />
          <Button label="RESET" onPress={act(() => kettle.reset())} />
          <Button label="PRESET" onPress={act(() => kettle.pressPreset())} />
          <Button label="LOCK" onPress={act(() => kettle.pressLock())} />
        </View>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, backgroundColor: C.bg, justifyContent: 'center' },
  panel: { margin: 16, padding: 14, gap: 16, borderRadius: 14, backgroundColor: C.panel },
  screen: { padding: 16, gap: 8, borderRadius: 8, backgroundColor: C.screen },
  row: { flexDirection: 'row', gap: 16 },
  label: { width: 80, color: C.muted },
  mono: { fontFamily: MONO, fontSize: 15 },
  bold: { fontWeight: '700' },
  light: { width: 14, height: 14, borderRadius: 7 },
  badge: { paddingHorizontal: 6, borderWidth: 1, borderRadius: 3 },
  dialLabel: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 6 },
  track: { height: 28, borderRadius: 6, borderWidth: 1, borderColor: C.edge, backgroundColor: C.control, overflow: 'hidden' },
  fill: { position: 'absolute', left: 0, top: 0, bottom: 0, backgroundColor: C.caution },
  mark: { position: 'absolute', top: 0, bottom: 0, width: 1, backgroundColor: C.text },
  buttons: { flexDirection: 'row', gap: 8 },
  button: { flex: 1, paddingVertical: 12, alignItems: 'center', borderRadius: 6, borderWidth: 1, borderColor: C.edge, backgroundColor: C.control },
});
