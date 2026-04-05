# Virtual LCD Firmware Simulator

SDK para simular display LCD em Rust.

## Arquitetura:

- `lcd-core`: estado do display, framebuffer, timing e comandos.
- `lcd-sdk`: API usada pelos exemplos como se fosse o driver do hardware.
- `lcd-renderer`: janela Linux que desenha o framebuffer dentro das molduras SVG da pasta `frames/`.

## Execução de testes

```bash
cargo test
```

## Exemplos disponíveis

### `dashboard`

Painel técnico com radar, barras, gráfico e cartões de status.

```bash
cargo run -p lcd-examples --bin dashboard
```

![Saída do exemplo dashboard](imgs/img.png)

### `oscilloscope`

Grade de medição com três ondas animadas.

```bash
cargo run -p lcd-examples --bin oscilloscope
```

![Saída do exemplo oscilloscope](imgs/img_1.png)

### `startup`

Tela de inicialização com anéis, órbitas e barra de progresso.

```bash
cargo run -p lcd-examples --bin startup
```

![Saída do exemplo startup](imgs/img_2.png)

### `gameboy`

Boot monocromático simples, com tela verde e descida da palavra `NINTENDO`.

```bash
cargo run -p lcd-examples --bin gameboy
```

![Saída do exemplo gameboy](imgs/img_3.png)

### `scripted`

Executa um arquivo de texto com comandos simples de desenho.

```bash
cargo run -p lcd-examples --bin scripted -- lcd-examples/scripts/panel.lcd
```

![Saída do exemplo scripted](imgs/img_4.png)

## Molduras SVG

As molduras ficam em `frames/` e são usadas só como entrada visual do renderer. Hoje o projeto já traz opções para:

- `1:1`
- `4:3`
- `16:9`
- `21:9`
- `9:16`

O renderer escolhe a moldura pelo aspect ratio do LCD e desenha a imagem útil dentro da área interna do SVG.

## Scripts de LCD

O bin `scripted` lê um arquivo texto linha por linha e converte isso em chamadas para o LCD. O arquivo de exemplo está em `lcd-examples/scripts/panel.lcd`.

Comandos suportados:

- `canvas <largura> <altura>`
- `frame auto|handheld`
- `clear r g b`
- `gradient x y w h r1 g1 b1 r2 g2 b2`
- `fill_rect x y w h r g b`
- `rect x y w h r g b`
- `line x0 y0 x1 y1 r g b`
- `circle cx cy raio r g b`
- `text x y escala r g b MENSAGEM`

## Estrutura

```text
lcd-core/
lcd-sdk/
lcd-renderer/
lcd-examples/
frames/
imgs/
```
