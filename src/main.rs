#![no_std]
#![no_main]
// #![deny(warnings)]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_probe;
extern crate rtic;
extern crate stm32g0xx_hal as hal;

mod strip;

use crate::strip::*;
use hal::time::*;
use hal::timer::*;
use hal::{analog::adc::*, prelude::*};
use hal::{gpio::*, watchdog::IndependedWatchdog};
use hal::{rcc, spi, stm32};
use infrared::{protocols::Nec, PeriodicReceiver};
use smart_leds::SmartLedsWrite;
use ws2812_spi::Ws2812;

const STRIP_SIZE: usize = 10;

pub use defmt_rtt as _;

type IrPin = gpioa::PA11<Input<Floating>>;
type SampleTimer = Timer<stm32::TIM1>;
type AnimationTimer = Timer<stm32::TIM16>;
type SpiLink = spi::Spi<stm32::SPI1, (spi::NoSck, spi::NoMiso, gpioa::PA12<DefaultMode>)>;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        strip: Strip,
    }

    #[local]
    struct Local {
        adc: Adc,
        link: Ws2812<SpiLink>,
        ir: PeriodicReceiver<Nec, IrPin>,
        animation_timer: AnimationTimer,
        sample_timer: SampleTimer,
        watchdog: IndependedWatchdog,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let pll_cfg = rcc::PllConfig::with_hsi(4, 24, 2);
        let rcc_cfg = rcc::Config::pll().pll_cfg(pll_cfg);
        let mut rcc = ctx.device.RCC.freeze(rcc_cfg);
        let mut adc = ctx.device.ADC.constrain(&mut rcc);
        adc.set_sample_time(SampleTime::T_80);

        let mut sample_timer = ctx.device.TIM1.timer(&mut rcc);
        sample_timer.start(50.micros());
        sample_timer.listen();

        let mut animation_timer = ctx.device.TIM16.timer(&mut rcc);
        animation_timer.start(160.millis());
        animation_timer.listen();

        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let spi = ctx.device.SPI1.spi(
            (spi::NoSck, spi::NoMiso, port_a.pa12),
            spi::MODE_0,
            3.MHz(),
            &mut rcc,
        );

        let ir = PeriodicReceiver::new(port_a.pa11.into_floating_input(), 20_000);
        let link = Ws2812::new(spi);
        let strip = Strip::new();

        let mut watchdog = ctx.device.IWDG.constrain();
        watchdog.start(100.millis());
        defmt::info!("Init completed");

        (
            Shared { strip },
            Local {
                adc,
                animation_timer,
                ir,
                link,
                sample_timer,
                watchdog,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM1_BRK_UP_TRG_COM, local = [ir, sample_timer], shared = [strip])]
    fn sample_timer_tick(ctx: sample_timer_tick::Context) {
        match ctx.local.ir.poll() {
            Ok(Some(cmd)) => {
                if cmd.addr == 0 {
                    defmt::info!("IR Command: {} {} {}", cmd.addr, cmd.cmd, cmd.repeat);
                    let mut strip = ctx.shared.strip;
                    strip.lock(|strip| strip.handle_command(cmd.cmd));
                }
            }
            Err(err) => {
                defmt::info!("ERR {}", err);
            }
            _ => {}
        }
        ctx.local.sample_timer.clear_irq();
    }

    #[task(binds = TIM16, local = [adc, link, animation_timer, watchdog], shared = [strip])]
    fn animation_timer_tick(ctx: animation_timer_tick::Context) {
        let temp = ctx.local.adc.read_temperature().unwrap();
        let mut strip = ctx.shared.strip;
        let animation = strip.lock(|strip| {
            if temp > 50 {
                defmt::info!("Overheat: {}", temp);
                strip.handle_overheat();
            } else {
                strip.handle_frame();
            }
            strip.animate()
        });
        ctx.local.link.write(animation).ok();

        ctx.local.animation_timer.clear_irq();
        ctx.local.watchdog.feed();
    }
}
