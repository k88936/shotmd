//
// Created by kvto on 9/15/24.
//

// You may need to build the project (run Qt uic code generator) to get "ui_shotmd.h" resolved

#include <QScreen>
#include <QMouseEvent>
#include <QPen>
#include <QPainter>
#include <QBuffer>
#include <QClipboard>
#include "shotmd.h"
#include "ui_shotmd.h"


shotmd::shotmd(QWidget *parent) :
        QMainWindow(parent), ui(new Ui::shotmd) {
    setWindowFlags(Qt::WindowStaysOnTopHint | Qt::FramelessWindowHint);   // 窗口置顶 + 隐藏标题栏
    QScreen *screen = QGuiApplication::primaryScreen();
    originalPixmap = screen->grabWindow(0);
    screenshotLabel = new QLabel(this);
    screenshotLabel->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    screenshotLabel->setAlignment(Qt::AlignCenter);
    screenshotLabel->setFixedSize(screen->geometry().width(), screen->geometry().height());
    screenshotLabel->setPixmap(originalPixmap);
    screenshotLabel->show();
    ui->setupUi(this);
    state = init;
//    connect(this,&shotmd::mousePressEvent,this,&shotmd::mousePressEvent);
}

void shotmd::mousePressEvent(QMouseEvent *event) {
    if (event->button() == Qt::RightButton) {
        QGuiApplication::exit();
    }
    switch (state) {
        case init:
            anchor = event->pos();
            state = anchorFirst;
            break;
    }

}

void shotmd::mouseMoveEvent(QMouseEvent *event) {
    switch (state) {
        case anchorFirst:
            anchor2 = event->pos();
            QPen pen;
            pen.setColor(Qt::red);
            pen.setWidth(2);
            QPixmap pixmap = originalPixmap.copy();
            QPainter painter(&pixmap);
            painter.setPen(pen);
            painter.drawRect(QRect(anchor, anchor2));
            painter.save();
            screenshotLabel->setPixmap(pixmap);
    }

}

void shotmd::mouseReleaseEvent(QMouseEvent *event) {
    switch (state) {
        case anchorFirst:
            QPixmap finalPixmap = originalPixmap.copy(QRect(anchor, event->pos()));
            finalPixmap.save("screenshot.png");
            QByteArray data;
            QBuffer buffer(&data);
            finalPixmap.save(&buffer, "PNG");
            data = data.toBase64();
            QGuiApplication::clipboard()->setText(data);
            QGuiApplication::exit();
    }
}

shotmd::~shotmd() {
    delete ui;
}
