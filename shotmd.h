//
// Created by kvto on 9/15/24.
//

#ifndef SHOTMD_SHOTMD_H
#define SHOTMD_SHOTMD_H

#include <QMainWindow>
#include <QLabel>


QT_BEGIN_NAMESPACE
namespace Ui { class shotmd; }
QT_END_NAMESPACE

class shotmd : public QMainWindow {
Q_OBJECT

public:
    explicit shotmd(QWidget *parent = nullptr);

    ~shotmd() override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    enum states {
        init, anchorFirst, anchorSecond, finish
    };
    QPixmap originalPixmap;
    QPoint anchor, anchor2;
    states state;
    QLabel *screenshotLabel;
    Ui::shotmd *ui;

    void mousePressEvent(QMouseEvent *event);

    void mouseReleaseEvent(QMouseEvent *event);

    void mouseMoveEvent(QMouseEvent *event);
void shot(QPoint Tl, QPoint RD);
};


#endif //SHOTMD_SHOTMD_H
